#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use codec::{Decode, Encode, Error as codecErr, HasCompact, Input, Output};
use rstd::{
    convert::{TryFrom, TryInto},
    prelude::*,
    result,
};
#[allow(unused_imports)]
use sp_runtime::traits::{
    Bounded, CheckedAdd, CheckedMul, CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member,
    One, Saturating, SimpleArithmetic, Zero,
};
use support::{
    decl_event, decl_module, decl_storage,
    dispatch::{Parameter, Result as DispatchResult},
    ensure,
    weights::SimpleDispatchInfo,
};
#[allow(unused_imports)]
use system::{ensure_root, ensure_signed, Error};

mod mock;
mod tests;

const DAY_IN_MILLI: u32 = 86400_000;
const RESERVED_MINT_RATIO: u32 = 6500;
const RESERVED_MINT_DIV: u32 = 10000;

pub type PhaseId = u32;

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PhaseInfo<Balance, AssetId> {
    pub id: PhaseId,
    pub quota: Balance,
    pub exchange: Balance,
    pub iou_asset_id: Option<AssetId>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct IOU<AccountId, Balance, AssetId> {
    pub asset_id: AssetId,
    pub balance: Balance,
    pub owner: AccountId,
}

#[derive(Encode, Decode, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ReleaseTrigger {
    PhaseChange,

    // not implemented
    BlockNumber(u64),
}
impl Default for ReleaseTrigger {
    fn default() -> Self {
        ReleaseTrigger::PhaseChange
    }
}

pub trait ReleasePack {
    type Balance;
    type AssetId;
    type AccountId;

    fn is_empty(&self) -> bool;
    fn release(&mut self) -> Option<Self::Balance>;
    fn get_asset_id(&self) -> Self::AssetId;
    fn get_owner(&self) -> Self::AccountId;
    fn check_release_trigger(&self, t: &ReleaseTrigger) -> bool;
}

#[derive(Encode, Decode, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ShareReleasePack<Balance, AssetId, AccountId> {
    pub asset_id: AssetId,
    pub owner: AccountId,
    pub phase_id: PhaseId,
    pub empty: bool,
    pub major: SharePackage<Balance>,
    pub minor: Option<SharePackage<Balance>>,
    pub release_trigger: ReleaseTrigger,
}

impl<Balance, AssetId, AccountId> ReleasePack for ShareReleasePack<Balance, AssetId, AccountId>
where
    Balance: Encode
        + Decode
        + Parameter
        + Member
        + SimpleArithmetic
        + Default
        + Copy
        + MaybeSerializeDeserialize,
    AssetId: Encode + Decode + Parameter + Member + SimpleArithmetic + Default + Copy,
    AccountId: Encode
        + Decode
        + Parameter
        + Member
        + MaybeSerializeDeserialize
        + MaybeDisplay
        + Ord
        + Default,
{
    type Balance = Balance;
    type AssetId = AssetId;
    type AccountId = AccountId;

    fn is_empty(&self) -> bool {
        self.empty
    }
    fn get_asset_id(&self) -> AssetId {
        self.asset_id
    }
    fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }
    fn release(&mut self) -> Option<Balance> {
        if self.empty {
            return None;
        }
        if self.major.terms_left > 0 {
            self.major.terms_left -= 1;
            self.empty = self.major.terms_left == 0
                && self.minor.as_ref().map_or(true, |v| v.terms_left == 0);
            return Some(self.major.per_term);
        }
        if let Some(ref mut minor) = self.minor {
            if minor.terms_left > 0 {
                minor.terms_left -= 1;
                self.empty = minor.terms_left == 0;
                return Some(minor.per_term);
            }
        }
        self.empty = true;
        return None;
    }
    fn check_release_trigger(&self, t: &ReleaseTrigger) -> bool {
        &self.release_trigger == t
    }
}

impl<Balance, AssetId, AccountId> ShareReleasePack<Balance, AssetId, AccountId>
where
    Balance: Encode
        + Decode
        + Parameter
        + Member
        + SimpleArithmetic
        + Default
        + Copy
        + MaybeSerializeDeserialize,
    AssetId: Encode + Decode + Parameter + Member + SimpleArithmetic + Default + Copy,
    AccountId: Encode
        + Decode
        + Parameter
        + Member
        + MaybeSerializeDeserialize
        + MaybeDisplay
        + Ord
        + Default,
{
    pub fn terms_left(&self) -> u32 {
        self.major.terms_left + self.minor.map_or(0, |v| v.terms_left)
    }
    pub fn get_total_balance(&self) -> Balance {
        if self.empty {
            Default::default()
        } else {
            self.major.get_total_balance()
                + self
                    .minor
                    .map_or(Default::default(), |m| m.get_total_balance())
        }
    }
}

#[derive(Encode, Decode, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct SharePackage<Balance> {
    pub terms_total: u32,
    pub terms_left: u32,
    pub per_term: Balance,
}

impl<B> SharePackage<B>
where
    B: Encode
        + Decode
        + Parameter
        + Member
        + SimpleArithmetic
        + Default
        + Copy
        + MaybeSerializeDeserialize,
{
    pub fn get_total_balance(&self) -> B {
        B::from(self.terms_left) * self.per_term
    }
}

#[allow(dead_code)]
enum BalanceChange<Balance> {
    Positive(Balance),
    Negative(Balance),
}
#[allow(dead_code)]
impl<B> BalanceChange<B>
where
    B: Copy,
{
    fn is_positive(&self) -> bool {
        match self {
            Self::Positive(_) => true,
            _ => false,
        }
    }
    fn is_negative(&self) -> bool {
        match self {
            Self::Negative(_) => true,
            _ => false,
        }
    }
    fn get_value(&self) -> B {
        match self {
            Self::Positive(v) => *v,
            Self::Negative(v) => *v,
        }
    }
}

pub trait Trait:
    system::Trait + assets::Trait + sudo::Trait + timestamp::Trait + price::Trait
{
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as Saving {
        /// the asset that user saves into our program
        CollectionAssetId get(collection_asset_id) config() : T::AssetId;

        /// the account where user saves go and it can be either a normal account which held by us or a totally random account
        /// probably need to be supervised by the public
        CollectionAccountId get(collection_account_id) build(|config: &GenesisConfig<T>| {
            config.collection_account_id.clone()
        }) : T::AccountId;

        /// the asset that is sent to user when he is creating a saving
        /// used for bonus distribution
        ShareAssetId get(share_asset_id) config() : T::AssetId;

        /// when a user wants to redeem a saving, he might choose to just transfer some "ShareAsset" into "CollectionAccount" directly
        /// and this amount of balance should be taken into account in his following actions
        ShareAssetCollected get(share_asset_collected) : map T::AccountId => T::Balance;

        /// each phase has its own asset, "IOUAsset"
        /// this holds a map as "IOUAsset" => "Phase"
        IOUAssetPhaseId get(iou_asset_phase_id) : map T::AssetId => PhaseId;

        /// identify ongoing phase
        CurrentPhaseId get(current_phase_id) config() : PhaseId;

        /// already used quota of each phase
        /// reset every time a new phase starts
        QuotaUsed get(used_quota) : T::Balance;

        /// keep tracking of how many phases left
        NumOfPhasesLeft get(num_of_phases_left) build(|config: &GenesisConfig<T>| {
            config.phase_infos.len() as u32
        }) : u32;

        /// *constant* should only be set by the genesis
        NumOfPhases get(num_of_phases) build(|config: &GenesisConfig<T>| {
            config.phase_infos.len() as u32
        }) : u32;

        /// info of all the phases
        /// phase id starts from 1
        PhaseInfos get(phase_info) build(|config: &GenesisConfig<T>| {
            config.phase_infos.iter().enumerate()
                .map(|(id, &(quota, exchange, asset_id))| {
                    <IOUAssetPhaseId<T>>::insert(&asset_id, (id+1) as PhaseId);
                    ((id+1) as PhaseId, PhaseInfo{id: (id+1) as PhaseId, quota: quota, exchange: exchange, iou_asset_id: Some(asset_id)})
                }).collect::<Vec<_>>()
        }) : linked_map PhaseId => PhaseInfo<T::Balance, T::AssetId>;

        /// the locked portion of user savings
        /// linked_map contains a Vec<ShareReleasePack>
        /// user would have only a piece of record for a single phase, when creating savings, all records within the same phase will be aggregated
        ShareUnreleasedList get(account_future_releases) : linked_map T::AccountId => Vec<ShareReleasePack<T::Balance, T::AssetId, T::AccountId>>;

        /// tracking share asset movement
        AccountShares get(account_shares) : linked_map T::AccountId => T::Balance;

        /// share asset total circulation
        SharesCirculation get(shares_circulation) : T::Balance;

        /// module level switch
        Paused get(paused) : bool = false;

        /// when current block timestamp minus "LastBonusTime" is over 86400 seconds, that's the checkpoint to calculate and release bonus
        LastBonusTime get(last_bonus_time) build(|_config: &GenesisConfig<T>| {
            <timestamp::Module<T>>::get()
        }) : T::Moment;

        /// use "ProfitAsset" for bonus
        ProfitAssetId get(profit_asset_id) config() : T::AssetId;

        /// use a specific account as "ProfitPool"
        /// might be supervised by the public
        ProfitPool get(profit_pool) config() : T::AccountId;

        /// TeamAccount take a 20% cut from daily profit bonus
        TeamAccountId get(team_account_id) config() : T::AccountId;

        /// Mint TBD to this account when saving
        ReservedMintWallet get(reserved_mint_wallet) config() : T::AccountId;

        /// TBD asset id
        ReservedMintAssetId get(reserved_mint_asset_id) config() : T::AssetId;
    }

    add_extra_genesis {
        config(collection_account_id): T::AccountId;
        // geneis configs of saving phases, keep unchanged to the end
        config(phase_infos): Vec<(T::Balance, T::Balance, T::AssetId)>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        fn on_initialize(_height: T::BlockNumber) {
            if !Self::paused() {
                if Self::check_bonus_time() {
                    Self::dispatch_bonus();
                }
            }
        }

        #[weight = SimpleDispatchInfo::MaxNormal]
        pub fn pause(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = true);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::MaxNormal]
        pub fn resume(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = false);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_share_asset_id(origin, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<assets::Module<T>>::asset_exists(&asset_id), "invalid asset id for saving share asset");
            <ShareAssetId<T>>::put(asset_id);
            Ok(())
        }

        // *** Caution
        // set_current_phase_id may break all saving schedule
        // ***
        // pub fn set_current_phase_id(origin, new_phase_id: PhaseId) -> DispatchResult {
        //     let _from = ensure_root(origin)?;
        //     ensure!(<PhaseInfos<T>>::exists(new_phase_id), "Invalid phase id for Saving");
        //     let old_phase_id = Self::get_current_phase_id();
        //     CurrentPhaseId::put(new_phase_id);
        //     Self::deposit_event(RawEvent::PhaseChanged(old_phase_id, new_phase_id));
        //     Ok(())
        // }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_iou_asset_id_for_phase(origin, phase_id: PhaseId, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<PhaseInfos<T>>::exists(&phase_id), "invalid phase id for saving");
            ensure!(<assets::Module<T>>::asset_exists(&asset_id), "invalid iou asset id for saving");
            if <IOUAssetPhaseId<T>>::exists(&asset_id) {
                <IOUAssetPhaseId<T>>::remove(&asset_id);
            }
            <PhaseInfos<T>>::mutate(phase_id, |pi| {
                pi.iou_asset_id = Some(asset_id);
            });
            <IOUAssetPhaseId<T>>::insert(&asset_id, phase_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_collection_account(origin, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <CollectionAccountId<T>>::put(account_id.clone());
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_collection_asset_id(origin, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<assets::Module<T>>::asset_exists(&asset_id), "invalid collection asset id");
            <CollectionAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_profit_asset_id(origin, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<assets::Module<T>>::asset_exists(&asset_id), "invalid collection asset id");
            <ProfitAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_profit_pool(origin, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <ProfitPool<T>>::put(account_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_team_account_id(origin, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <TeamAccountId<T>>::put(account_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn staking(origin, asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            let collection_account_id = Self::collection_account_id();
            ensure!(<CollectionAssetId<T>>::get() == asset_id, "can't collect this asset");
            ensure!(<assets::Module<T>>::free_balance(&asset_id, &who) >= amount, "insufficient balance");
            let staking_balance = Self::create_staking(who.clone(), amount)?;
            <assets::Module<T>>::make_transfer_with_event(&asset_id, &who, &collection_account_id, staking_balance)?;
            Self::create_reserved(Self::sbtc_to_reserved_mint(staking_balance))
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn sudo_staking(origin, asset_id: T::AssetId, amount: T::Balance, delegatee: T::AccountId) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            let collection_account_id = Self::collection_account_id();
            ensure!(<CollectionAssetId<T>>::get() == asset_id, "can't collect this asset");
            ensure!(<assets::Module<T>>::free_balance(&asset_id, &delegatee) >= amount, "insufficient balance");
            let staking_balance = Self::create_staking(delegatee.clone(), amount)?;
            <assets::Module<T>>::make_transfer_with_event(&asset_id, &delegatee, &collection_account_id, staking_balance)?;
            Self::create_reserved(Self::sbtc_to_reserved_mint(staking_balance))
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn redeem(origin, iou_asset_id: T::AssetId, iou_asset_amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            let share_asset_id = Self::share_asset_id();
            let collection_asset_id = Self::collection_asset_id();
            let collection_account_id = Self::collection_account_id();
            ensure!(!share_asset_id.is_zero(), "fail to find share asset id");
            ensure!(!collection_asset_id.is_zero(), "fail to find collection asset id");
            ensure!(<IOUAssetPhaseId<T>>::exists(&iou_asset_id), "no such contract assets");
            let burn_reserved = Self::sbtc_to_reserved_mint(iou_asset_amount);
            ensure!(<assets::Module<T>>::free_balance(&Self::reserved_mint_asset_id(), &Self::reserved_mint_wallet()) >= burn_reserved, "reserved wallet is short");
            Self::check_can_redeem(iou_asset_id.clone(), who.clone(), iou_asset_amount)?;
            Self::burn_reserved(burn_reserved)?;
            <assets::Module<T>>::make_transfer_with_event(&iou_asset_id, &who, &collection_account_id, iou_asset_amount)?;
            Self::make_redeem(
                &iou_asset_id,
                &who,
                &collection_asset_id,
                &collection_account_id,
                iou_asset_amount,
                &share_asset_id,
            ).or_else(|err| -> DispatchResult {
                <assets::Module<T>>::make_transfer_with_event(&iou_asset_id, &collection_account_id, &who, iou_asset_amount)?;
                Err(err)
            })?;
            <assets::Module<T>>::burn(system::RawOrigin::Root.into(), iou_asset_id.clone(), collection_account_id.clone(), iou_asset_amount)
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn sudo_redeem(origin, iou_asset_id: T::AssetId, iou_asset_amount: T::Balance, delegatee: T::AccountId) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            let share_asset_id = Self::share_asset_id();
            let collection_asset_id = Self::collection_asset_id();
            let collection_account_id = Self::collection_account_id();
            ensure!(!share_asset_id.is_zero(), "fail to find share asset id");
            ensure!(!collection_asset_id.is_zero(), "fail to find collection asset id");
            ensure!(<IOUAssetPhaseId<T>>::exists(&iou_asset_id), "no such contract assets");
            Self::check_can_redeem(iou_asset_id.clone(), delegatee.clone(), iou_asset_amount)?;
            <assets::Module<T>>::make_transfer_with_event(&iou_asset_id, &delegatee, &collection_account_id, iou_asset_amount)?;
            Self::make_redeem(
                &iou_asset_id,
                &delegatee,
                &collection_asset_id,
                &collection_account_id,
                iou_asset_amount,
                &share_asset_id,
            ).or_else(|err| -> DispatchResult {
                <assets::Module<T>>::make_transfer_with_event(&iou_asset_id, &collection_account_id, &delegatee, iou_asset_amount)?;
                Err(err)
            })?;
            <assets::Module<T>>::burn(system::RawOrigin::Root.into(), iou_asset_id.clone(), collection_account_id.clone(), iou_asset_amount)
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn force_release_bonus(origin) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            Self::dispatch_bonus();
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Immutable
    /// asset_id is the IOU asset that the transaction carries
    pub fn check_can_redeem(
        asset_id: T::AssetId,
        who: T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        let share_asset_id = Self::share_asset_id();

        if !share_asset_id.is_zero() {
            let phase_id = <IOUAssetPhaseId<T>>::get(&asset_id);
            let phase_info = Self::phase_info(phase_id);
            let returned_share_asset = <ShareAssetCollected<T>>::get(&who);
            let free_share_asset = <assets::Module<T>>::free_balance(&share_asset_id, &who);
            let required_share_asset = amount * phase_info.exchange;
            let num_of_phases = Self::num_of_phases();
            let num_of_phases_left = Self::num_of_phases_left();
            let (free_share_asset_required, locked_share_asset_required) =
                Self::redeem_required_balances_in_ratio(
                    phase_id,
                    required_share_asset,
                    num_of_phases,
                    num_of_phases_left,
                );

            // it's ok if potentially the redemption will deduct all from free balance
            if free_share_asset + returned_share_asset >= required_share_asset {
                return Ok(());
            }

            // at last we only need to make sure the total balance can cover the redemption
            let unreleased_list = <ShareUnreleasedList<T>>::get(&who);
            let unreleased = unreleased_list
                .iter()
                .filter(|v| !v.empty && v.asset_id == share_asset_id && v.phase_id == phase_id)
                .fold(T::Balance::zero(), |acc, v| acc + v.get_total_balance());

            if free_share_asset + returned_share_asset < free_share_asset_required {
                if free_share_asset_required + locked_share_asset_required
                    != unreleased + (free_share_asset + returned_share_asset)
                {
                    return Err("not enough available share asset");
                }
            }

            if required_share_asset > returned_share_asset + unreleased + free_share_asset {
                return Err("exceed allowed redeem limits");
            }
        }

        Ok(())
    }

    /// make_redeem should only be called after check_can_redeem immediately
    /// asset_id is the IOU asset id which user has transferred to our collection account
    /// who is the user transferring the asset
    /// collection_asset_id
    pub fn make_redeem(
        iou_asset_id: &T::AssetId,
        who: &T::AccountId,
        collection_asset_id: &T::AssetId,
        collection_account: &T::AccountId,
        amount: T::Balance,
        share_asset_id: &T::AssetId,
    ) -> DispatchResult {
        let phase_id = <IOUAssetPhaseId<T>>::get(iou_asset_id);
        let phase_info = Self::phase_info(phase_id);
        let returned_share_asset = <ShareAssetCollected<T>>::get(who);
        let share_asset_required = amount * phase_info.exchange;
        let num_of_phases = Self::num_of_phases();
        let num_of_phases_left = Self::num_of_phases_left();
        let (free_share_asset_required, locked_share_asset_required) =
            Self::redeem_required_balances_in_ratio(
                phase_id,
                share_asset_required,
                num_of_phases,
                num_of_phases_left,
            );
        let mut share_asset_need_to_burn = T::Balance::zero();
        let mut free_share_asset_to_deduct = free_share_asset_required;
        let mut locked_share_asset_to_deduct = locked_share_asset_required;

        // the balance of share asset returned in advance takes the highest priority
        if returned_share_asset > T::Balance::zero() {
            if free_share_asset_to_deduct > returned_share_asset {
                <ShareAssetCollected<T>>::remove(who);
                // these have already been in the collection account, just burn
                share_asset_need_to_burn += returned_share_asset;
                // keep tracking the remaining amount
                free_share_asset_to_deduct -= returned_share_asset;
            } else {
                let returned_left = returned_share_asset - free_share_asset_to_deduct;
                share_asset_need_to_burn += free_share_asset_to_deduct;
                free_share_asset_to_deduct = T::Balance::zero();

                if returned_left > locked_share_asset_to_deduct {
                    // deduct all locked portion from returned
                    <ShareAssetCollected<T>>::mutate(who, |v| {
                        *v = returned_left - locked_share_asset_to_deduct;
                    });
                    share_asset_need_to_burn += locked_share_asset_to_deduct;
                    locked_share_asset_to_deduct = T::Balance::zero();
                } else {
                    <ShareAssetCollected<T>>::remove(who);
                    share_asset_need_to_burn += returned_left;
                    locked_share_asset_to_deduct -= returned_left;
                }
            }
        }

        // lcoked_imbalance will only be helpful when required_locked_balance is more than the actual locked balance
        let mut locked_short_balance = T::Balance::zero();
        let mut free_short_balance = T::Balance::zero();
        let unrel_list = &<ShareUnreleasedList<T>>::take(who);
        let unrel_total_balance: T::Balance = unrel_list
            .iter()
            .filter(|v| v.phase_id == phase_id && v.owner == who.clone())
            .fold(T::Balance::zero(), |accu, v| accu + v.get_total_balance());
        // only when calculation with both unreleased balance & locked balance required would cause some imbalance
        if unrel_total_balance < locked_share_asset_to_deduct {
            locked_short_balance = locked_share_asset_to_deduct - unrel_total_balance;
            locked_share_asset_to_deduct = unrel_total_balance;
        }

        if free_share_asset_to_deduct > T::Balance::zero() {
            let free_share_asset = <assets::Module<T>>::free_balance(share_asset_id, who);
            ensure!(
                free_share_asset + unrel_total_balance >= share_asset_required,
                "total balance doesn't match"
            );

            if free_share_asset < free_share_asset_to_deduct {
                free_short_balance = free_share_asset_to_deduct - free_share_asset;
                free_share_asset_to_deduct = free_share_asset;
            }
        }

        free_share_asset_to_deduct += locked_short_balance;
        locked_share_asset_to_deduct += free_short_balance;

        ensure!(
            free_share_asset_to_deduct + locked_share_asset_to_deduct + returned_share_asset
                == free_share_asset_required + locked_share_asset_required,
            "in & out don't match"
        );

        <assets::Module<T>>::make_transfer_with_event(
            share_asset_id,
            who,
            collection_account,
            free_share_asset_to_deduct,
        )?;
        // free_share_asset_required will be transfer from user's account to our collection account
        // which needs to be burnt also
        share_asset_need_to_burn += free_share_asset_to_deduct;

        let nl = Self::aggregate_phase_unreleased(
            &unrel_list,
            phase_id,
            share_asset_id,
            who,
            Some(BalanceChange::Negative(locked_share_asset_to_deduct)),
        )?;
        <ShareUnreleasedList<T>>::insert(who, nl);

        if !share_asset_need_to_burn.is_zero() {
            <assets::Module<T>>::burn(
                system::RawOrigin::Root.into(),
                share_asset_id.clone(),
                collection_account.clone(),
                share_asset_need_to_burn,
            )?;
        }

        <assets::Module<T>>::make_transfer_with_event(
            collection_asset_id,
            collection_account,
            who,
            amount,
        )?;

        Self::deposit_event(RawEvent::StakingRedeemed(
            who.clone(),
            share_asset_need_to_burn,
            *iou_asset_id,
            amount,
        ));

        Ok(())
    }

    fn aggregate_phase_unreleased(
        unreleased_list: &[ShareReleasePack<T::Balance, T::AssetId, T::AccountId>],
        phase_id: PhaseId,
        asset_id: &T::AssetId,
        who: &T::AccountId,
        change: Option<BalanceChange<T::Balance>>,
    ) -> Result<Vec<ShareReleasePack<T::Balance, T::AssetId, T::AccountId>>, &'static str> {
        let mut new_list: Vec<ShareReleasePack<T::Balance, T::AssetId, T::AccountId>> =
            Vec::with_capacity(unreleased_list.len());
        let mut unreleased_total_balance = T::Balance::zero();
        let mut terms_total = 0;
        let mut terms_left = 0;

        for i in unreleased_list {
            if i.asset_id != *asset_id || i.phase_id != phase_id {
                new_list.push(i.clone());
            } else {
                unreleased_total_balance += i.get_total_balance();
                if terms_total != i.major.terms_total {
                    terms_total = i.major.terms_total;
                    terms_left = i.terms_left();
                }
            }
        }

        if change.is_none() {
            let mut aggregated = Self::create_share_release_pack(
                who.clone(),
                *asset_id,
                phase_id,
                unreleased_total_balance,
                terms_left,
            );
            aggregated.major.terms_total = terms_total;
            new_list.push(aggregated);
        } else {
            let change = change.unwrap();
            if change.is_positive() {
                let mut aggregated = Self::create_share_release_pack(
                    who.clone(),
                    *asset_id,
                    phase_id,
                    unreleased_total_balance + change.get_value(),
                    terms_left,
                );
                aggregated.major.terms_total = terms_total;
                new_list.push(aggregated);
            } else {
                ensure!(
                    unreleased_total_balance >= change.get_value(),
                    "not enought locked balance"
                );
                if unreleased_total_balance > change.get_value() {
                    let mut aggregated = Self::create_share_release_pack(
                        who.clone(),
                        *asset_id,
                        phase_id,
                        unreleased_total_balance - change.get_value(),
                        terms_left,
                    );
                    aggregated.major.terms_total = terms_total;
                    new_list.push(aggregated);
                }
            }
        }

        Ok(new_list)
    }

    fn sbtc_to_reserved_mint(amount: T::Balance) -> T::Balance {
        let price_in_balance: T::Balance =
            TryFrom::<u128>::try_from(<price::Module<T>>::current_price())
                .ok()
                .unwrap();
        price_in_balance * amount * T::Balance::from(RESERVED_MINT_RATIO)
            / (T::Balance::from(price::PRICE_PRECISION) * T::Balance::from(RESERVED_MINT_DIV))
    }

    fn create_reserved(amount: T::Balance) -> DispatchResult {
        <assets::Module<T>>::mint(
            system::RawOrigin::Root.into(),
            Self::reserved_mint_asset_id(),
            Self::reserved_mint_wallet(),
            amount,
        )
    }

    fn burn_reserved(amount: T::Balance) -> DispatchResult {
        <assets::Module<T>>::burn(
            system::RawOrigin::Root.into(),
            Self::reserved_mint_asset_id(),
            Self::reserved_mint_wallet(),
            amount,
        )
    }

    /// create_staking accepts accountid and 'BTC' saving balance
    /// will create a saving vesting schedule according to the current phase
    pub fn create_staking(
        who: T::AccountId,
        balance: T::Balance,
    ) -> result::Result<T::Balance, &'static str> {
        ensure!(!balance.is_zero(), "saving can't be zero");

        let share_asset_id = Self::share_asset_id();
        let phase_id = Self::current_phase_id();

        ensure!(
            <PhaseInfos<T>>::exists(&phase_id),
            "current phase id is invalid"
        );

        let phase_info = <PhaseInfos<T>>::get(&phase_id);
        let used_quota = Self::used_quota();
        let quota_will_be = used_quota.checked_add(&balance).expect("quota overflow");
        let mut next_phase = false;
        let mut next_phase_iou = <T::Balance>::zero();

        let mut iou_balance = if quota_will_be <= phase_info.quota {
            if quota_will_be == phase_info.quota {
                next_phase = true;
            } else {
                <QuotaUsed<T>>::put(quota_will_be);
            }
            balance
        } else {
            next_phase = true;
            next_phase_iou = balance - (phase_info.quota - used_quota);
            phase_info.quota - used_quota
        };

        // assets in iou are yet to be minted
        let iou = Self::create_iou(who.clone(), &phase_info, iou_balance);
        // assets in share_pack are yet to be minted
        let mut share_pack = Self::create_release_pack_for_saving(
            who.clone(),
            share_asset_id,
            iou_balance,
            &phase_info,
        )?;

        let (_, share_asset_balance) = Self::mint_assets(Some(&iou), Some(&mut share_pack))?;
        Self::register_iou(iou);
        Self::register_share_pack(share_pack);

        Self::deposit_event(RawEvent::StakingCreated(
            who.clone(),
            share_asset_balance,
            phase_info.iou_asset_id.unwrap(),
            iou_balance,
        ));

        if next_phase {
            let shifted = Self::shift_next_phase(phase_id);
            if shifted.is_some() && next_phase_iou > <T::Balance>::zero() {
                iou_balance += Self::create_staking(who.clone(), next_phase_iou)?;
            }
        }

        Ok(iou_balance)
    }

    /// mint_assets will call ShareReleasePack::release(&mut self), which cause some side-effect inside the share_pack
    fn mint_assets(
        iou_mapbe: Option<&IOU<T::AccountId, T::Balance, T::AssetId>>,
        share_pack_maybe: Option<&mut ShareReleasePack<T::Balance, T::AssetId, T::AccountId>>,
    ) -> result::Result<(T::Balance, T::Balance), &'static str> {
        let mut iou_balance = T::Balance::zero();
        let mut share_asset_balance = T::Balance::zero();
        if let Some(iou) = iou_mapbe {
            <assets::Module<T>>::mint(
                system::RawOrigin::Root.into(),
                iou.asset_id,
                iou.owner.clone(),
                iou.balance,
            )?;
            iou_balance = iou.balance;
        }
        if let Some(share_pack) = share_pack_maybe {
            if let Some(share_balance) = share_pack.release() {
                <assets::Module<T>>::mint(
                    system::RawOrigin::Root.into(),
                    share_pack.asset_id,
                    share_pack.owner.clone(),
                    share_balance,
                )?;
                share_asset_balance = share_balance;
            }
        }

        Ok((iou_balance, share_asset_balance))
    }

    /// For now, we assume:
    ///   the RS Contract Token is exactly 1:1 to BTC
    ///   RBTC is exactly 1:1 to Satoshi

    /// create_iou is called when a user deposit his BTC into saving program
    fn create_iou(
        owner: T::AccountId,
        phase_info: &PhaseInfo<T::Balance, T::AssetId>,
        saving: T::Balance,
    ) -> IOU<T::AccountId, T::Balance, T::AssetId> {
        IOU {
            asset_id: phase_info.iou_asset_id.unwrap(),
            balance: saving,
            owner: owner,
        }
    }

    /// register_iou will persist this piece of IOU on to the chain's storage
    fn register_iou(_iou: IOU<T::AccountId, T::Balance, T::AssetId>) {}

    /// register_share_pack persist this piece of share pack on to the chain's storage
    fn register_share_pack(share_pack: ShareReleasePack<T::Balance, T::AssetId, T::AccountId>) {
        if !share_pack.is_empty() {
            let owner = share_pack.owner.clone();
            if !<ShareUnreleasedList<T>>::exists(&owner) {
                <ShareUnreleasedList<T>>::insert(&owner, vec![share_pack]);
            } else {
                let mut v = <ShareUnreleasedList<T>>::take(&owner);
                let phase_id = share_pack.phase_id;
                let asset_id = share_pack.asset_id.clone();
                v.push(share_pack);
                let v = Self::aggregate_phase_unreleased(&v, phase_id, &asset_id, &owner, None)
                    .unwrap();
                <ShareUnreleasedList<T>>::insert(&owner, v);
            }
        }
    }

    /// create_release_pack_for_saving is called when a user deposit his BTC into saving program
    fn create_release_pack_for_saving(
        owner: T::AccountId,
        asset_id: T::AssetId,
        balance: T::Balance,
        phase_info: &PhaseInfo<T::Balance, T::AssetId>,
    ) -> Result<ShareReleasePack<T::Balance, T::AssetId, T::AccountId>, &'static str> {
        if balance.is_zero() {
            return Err("saving balance must not be zero");
        }

        let share_balance = balance
            .checked_mul(&phase_info.exchange)
            .expect("saving share overflow");

        let effective_phases_count = NumOfPhasesLeft::get();
        if effective_phases_count < 1 {
            return Err("should not create share packages for the last phase");
        }

        Ok(Self::create_share_release_pack(
            owner,
            asset_id,
            phase_info.id,
            share_balance,
            effective_phases_count,
        ))
    }

    fn create_share_release_pack(
        owner: T::AccountId,
        asset_id: T::AssetId,
        phase_id: PhaseId,
        balance: T::Balance,
        count: u32,
    ) -> ShareReleasePack<T::Balance, T::AssetId, T::AccountId> {
        let portion = if count == 1 {
            balance
        } else {
            (balance + T::Balance::from(count - 1)) / T::Balance::from(count)
        };
        let mut pack = ShareReleasePack {
            release_trigger: ReleaseTrigger::PhaseChange,
            empty: false,
            asset_id: asset_id,
            phase_id: phase_id,
            owner: owner,
            major: SharePackage {
                terms_left: count - 1,
                terms_total: count,
                per_term: portion,
            },
            minor: None,
        };
        let virtual_share_balance = portion * T::Balance::from(count);

        if balance == virtual_share_balance {
            pack.major.terms_left += 1;
        } else if balance > virtual_share_balance {
            pack.minor = Some(SharePackage {
                terms_total: 1,
                terms_left: 1,
                per_term: balance - virtual_share_balance,
            });
        } else {
            pack.minor = Some(SharePackage {
                terms_total: 1,
                terms_left: 1,
                per_term: portion - (virtual_share_balance - balance),
            });
        }

        pack
    }

    fn shift_next_phase(current_phase_id: PhaseId) -> Option<PhaseId> {
        let new_phase_id = current_phase_id
            .checked_add(1)
            .expect("next phase overflow");

        Self::shares_release_by_phase_change(current_phase_id)
            .expect("fail to mint released shares");

        if <PhaseInfos<T>>::exists(new_phase_id) {
            let empty_quota: T::Balance = Zero::zero();
            <QuotaUsed<T>>::put(empty_quota);
            CurrentPhaseId::put(new_phase_id);
            NumOfPhasesLeft::mutate(|v| {
                if *v > 0 {
                    *v = *v - 1;
                }
            });

            Self::deposit_event(RawEvent::PhaseChanged(current_phase_id, new_phase_id));
            return Some(new_phase_id);
        } else {
            <QuotaUsed<T>>::put(<PhaseInfos<T>>::get(current_phase_id).quota);
            return None;
        }
    }

    fn shares_release_by_phase_change(_from_phase: PhaseId) -> DispatchResult {
        <ShareUnreleasedList<T>>::enumerate().for_each(|(account_id, mut list)| {
            let list: Vec<ShareReleasePack<T::Balance, T::AssetId, T::AccountId>> = list
                .iter_mut()
                .map(|p| {
                    Self::mint_assets(None, Some(p)).unwrap();
                    p.clone()
                })
                .filter(|p| !p.is_empty())
                .collect();

            <ShareUnreleasedList<T>>::remove(&account_id);
            if list.len() > 0 {
                <ShareUnreleasedList<T>>::insert(&account_id, list);
            }
        });

        Ok(())
    }

    /// track_share_asset_movement will not check free balance
    /// this method is supposed to be called after checks like in the transfer hook
    fn track_share_asset_movement(
        from: &T::AccountId,
        to: &T::AccountId,
        balance: T::Balance,
    ) -> DispatchResult {
        let collection_account_id = Self::collection_account_id();
        if *from != collection_account_id {
            let from_balance = Self::account_shares(from);
            if from_balance < balance {
                Self::_pause(line!());
            } else if from_balance == balance {
                <AccountShares<T>>::remove(from);
            } else {
                <AccountShares<T>>::mutate(from, |v| {
                    *v -= balance;
                });
            }
        } else {
            <SharesCirculation<T>>::mutate(|v| {
                *v += balance;
            });
        }

        if *to != collection_account_id {
            if <AccountShares<T>>::exists(to) {
                <AccountShares<T>>::mutate(to, |v| {
                    let shares = v.checked_add(&balance);
                    if shares.is_none() {
                        Self::_pause(line!());
                    } else {
                        *v = shares.unwrap();
                    }
                });
            } else {
                <AccountShares<T>>::insert(to, balance);
            }
        } else {
            <SharesCirculation<T>>::mutate(|v| {
                let shares = v.checked_sub(&balance);
                if shares.is_none() {
                    Self::_pause(line!());
                } else {
                    *v = shares.unwrap();
                }
            });
        }

        Ok(())
    }

    /// this ratio should be exact with create_share_release_pack
    /// here, the ratio is not rigid
    fn redeem_required_balances_in_ratio(
        phase_id: PhaseId,
        required_balance: T::Balance,
        total_phases: u32,
        phases_left: u32,
    ) -> (T::Balance, T::Balance) {
        // both works, just for refs
        // let free_share_asset_required = required_balance
        //     * T::Balance::from(1 + total_phases - (phase_id - 1) - phases_left)
        //     / T::Balance::from(total_phases - phase_id + 1);
        // (
        //     free_share_asset_required,
        //     required_balance - free_share_asset_required,
        // )
        let locked_share_asset_required = required_balance * T::Balance::from(phases_left - 1)
            / T::Balance::from(total_phases - phase_id + 1);
        (
            required_balance - locked_share_asset_required,
            locked_share_asset_required,
        )
    }

    fn _pause(linum: u32) {
        Paused::mutate(|v| {
            *v = true;
        });
        Self::deposit_event(RawEvent::Paused(
            linum,
            <system::Module<T>>::block_number(),
            <system::Module<T>>::extrinsic_index().unwrap(),
        ));
    }

    fn check_bonus_time() -> bool {
        let last = Self::last_bonus_time();
        let now = <timestamp::Module<T>>::get();
        let day = T::Moment::from(DAY_IN_MILLI);
        if last.is_zero() {
            <LastBonusTime<T>>::put((now / day) * day);
            return false;
        }
        if now - last < day {
            return false;
        }
        <LastBonusTime<T>>::mutate(|v| {
            *v += day;
        });
        true
    }

    fn split_profit(amount: T::Balance) -> (T::Balance, T::Balance) {
        let users = amount * T::Balance::from(8) / T::Balance::from(10);
        (users, amount - users)
    }

    fn dispatch_bonus() {
        let team_account = Self::team_account_id();
        let profit_pool = Self::profit_pool();
        let profit_asset = Self::profit_asset_id();
        let total_profit = <assets::Module<T>>::free_balance(&profit_asset, &profit_pool);
        let circulation = Self::shares_circulation();
        let (users, teams) = Self::split_profit(total_profit);

        // give the team their bonus cut
        <assets::Module<T>>::make_transfer_with_event(
            &profit_asset,
            &profit_pool,
            &team_account,
            teams,
        )
        .unwrap_or_default();

        // give each user his bonus cut
        for (user_id, balance) in <AccountShares<T>>::enumerate() {
            let user_bonus = balance * users / circulation;
            <assets::Module<T>>::make_transfer_with_event(
                &profit_asset,
                &profit_pool,
                &user_id,
                user_bonus,
            )
            .unwrap_or_default();
        }

        Self::deposit_event(RawEvent::Bonus());
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        AssetId = <T as pallet_generic_asset::Trait>::AssetId,
        Balance = <T as pallet_generic_asset::Trait>::Balance,
        LineNumber = u32,
        ExtrinsicIndex = u32,
        BlockNumber = <T as system::Trait>::BlockNumber,
    {
        // fired when current phase changed
        PhaseChanged(PhaseId, PhaseId),
        ReleaseSavingShare(AccountId, Balance),

        // (AccountId, RBTC balance, Phase contract, Phase contract balance a.k.a SBTC balance)
        StakingCreated(AccountId, Balance, AssetId, Balance),
        StakingRedeemed(AccountId, Balance, AssetId, Balance),

        Paused(LineNumber, BlockNumber, ExtrinsicIndex),

        Bonus(),
    }
);

impl<T: Trait> assets::traits::OnAssetMint<T::AssetId, T::AccountId, T::Balance> for Module<T> {
    fn on_asset_mint(
        asset_id: &T::AssetId,
        to: &T::AccountId,
        balance: &T::Balance,
    ) -> DispatchResult {
        let share_asset_id = Self::share_asset_id();
        let collection_account_id = Self::collection_account_id();
        if share_asset_id == *asset_id && collection_account_id != *to {
            if <AccountShares<T>>::exists(to) {
                <AccountShares<T>>::mutate(to, |v| {
                    let shares = v.checked_add(&balance);
                    if shares.is_none() {
                        Self::_pause(line!());
                    } else {
                        *v = shares.unwrap();
                    }
                });
            } else {
                <AccountShares<T>>::insert(&to, balance);
            }
            <SharesCirculation<T>>::mutate(|v| {
                let shares = v.checked_add(&balance);
                if shares.is_none() {
                    Self::_pause(line!());
                } else {
                    *v = shares.unwrap();
                }
            });
            return Ok(());
        }
        Ok(())
    }
}

impl<T: Trait> assets::traits::OnAssetBurn<T::AssetId, T::AccountId, T::Balance> for Module<T> {
    fn on_asset_burn(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        balance: &T::Balance,
    ) -> DispatchResult {
        if Self::paused() {
            return Ok(());
        }

        let share_asset_id = Self::share_asset_id();
        let collection_account_id = Self::collection_account_id();
        if share_asset_id == *asset_id && collection_account_id != *from {
            <AccountShares<T>>::mutate(from, |v| {
                let shares = v.checked_sub(balance);
                if shares.is_none() {
                    Self::_pause(line!());
                } else {
                    *v = shares.unwrap();
                }
            });
            <SharesCirculation<T>>::mutate(|v| {
                let shares = v.checked_sub(balance);
                if shares.is_none() {
                    Self::_pause(line!());
                } else {
                    *v = shares.unwrap();
                }
            });
            return Ok(());
        }

        Ok(())
    }
}

impl<T: Trait> assets::traits::BeforeAssetTransfer<T::AssetId, T::AccountId, T::Balance>
    for Module<T>
{
    fn before_asset_transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        balance: &T::Balance,
    ) -> DispatchResult {
        if Self::paused() {
            return Ok(());
        }

        if <IOUAssetPhaseId<T>>::exists(&asset_id) && *to == Self::collection_account_id() {
            Self::check_can_redeem(*asset_id, from.clone(), *balance)?;
        }

        Ok(())
    }
}

/// THE transfer hook
/// where amazing happens
/// saving needs to be notified when a transfer happens in the assets module
impl<T: Trait> assets::traits::OnAssetTransfer<T::AssetId, T::AccountId, T::Balance> for Module<T> {
    fn on_asset_transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        balance: &T::Balance,
    ) -> DispatchResult {
        if Self::paused() {
            return Ok(());
        }

        ensure!(from != to, "from == to in transfer");
        let share_asset_id = <ShareAssetId<T>>::get();
        if share_asset_id.is_zero() {
            // share asset id is the core, our code would be dead without it
            return Ok(());
        }

        // share assets transfer between users,
        // we wanna keep tracking for calculating bonus
        if *asset_id == share_asset_id {
            return Self::track_share_asset_movement(from, to, *balance);
        }

        let collection_account_id = <CollectionAccountId<T>>::get();
        let collection_asset_id = <CollectionAssetId<T>>::get();
        // create a saving when a user wire his btc into our collection account
        if *asset_id == collection_asset_id && *to == collection_account_id {
            return Self::create_staking(from.clone(), *balance).map(|_: T::Balance| {});
        }

        // when user successfully wire some iou asset into our collection account
        // we consider this is a redeemal
        if <IOUAssetPhaseId<T>>::exists(asset_id) && *to == collection_account_id {
            return Self::make_redeem(
                asset_id,
                from,
                &collection_asset_id,
                to,
                *balance,
                &share_asset_id,
            );
        }

        Ok(())
    }
}
