#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use codec::{Decode, Encode, Error as codecErr, HasCompact, Input, Output};
use rstd::prelude::*;
#[allow(unused_imports)]
use rstd::{
    self,
    collections::btree_map,
    convert::{TryFrom, TryInto},
    marker::PhantomData,
    result,
};
#[allow(unused_imports)]
use support::{
    decl_event, decl_module, decl_storage,
    dispatch::{Parameter, Result as DispatchResult},
    ensure,
    traits::{Contains, Get},
    weights::SimpleDispatchInfo,
};
#[allow(unused_imports)]
use system::{ensure_root, ensure_signed, Error};

#[allow(unused_imports)]
use sp_runtime::traits::{
    Bounded, CheckedAdd, CheckedMul, CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member,
    One, Saturating, SignedExtension, SimpleArithmetic, Zero,
};
// use sp_runtime::{
// 	  transaction_validity::{
// 		    TransactionPriority, ValidTransaction, InvalidTransaction, TransactionValidityError,
// 		    TransactionValidity,
// 	  },
// }

pub use price::Price;

mod mock;
mod tests;

pub const INTEREST_RATE_PREC: u32 = 10000_0000;
pub const LTV_PREC: u32 = 10000;
pub const PRICE_PREC: u32 = price::PRICE_PRECISION;

/// should be 86400 seconds, a.k.a one day
pub const TERMS_UNIT: u32 = 86400;

/// in terms of TERMS_UNIT, a.k.a 2 days
pub const DUE_EXTEND: u32 = 2;

pub type LoanPackageId = u64;
pub type LoanId = u64;
pub type CreditLineId = u64;
pub type LTV = u64;

#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum LoanHealth {
    Well,
    Warning(LTV),
    Liquidating(LTV),
    Extended,
    Expired,
}
impl Default for LoanHealth {
    fn default() -> Self {
        Self::Well
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum LoanPackageStatus {
    Active,
    Inactive,
}
impl Default for LoanPackageStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CollateralLoan<Balance> {
    pub collateral_amount: Balance,
    pub loan_amount: Balance,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct LoanPackage<Balance, AssetId> {
    pub id: LoanPackageId,
    pub status: LoanPackageStatus,

    // days of our lives
    pub terms: u32,
    pub min: Balance,

    // per hour
    pub interest_rate_hourly: u32,
    pub collateral_asset_id: AssetId,
    pub loan_asset_id: AssetId,
}

impl<Balance, AssetId> LoanPackage<Balance, AssetId>
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
{
    pub fn get_interest(&self, amount: Balance) -> Balance {
        amount
            * Balance::from(self.terms)
            * Balance::from(24 as u32)
            * Balance::from(self.interest_rate_hourly)
            / Balance::from(INTEREST_RATE_PREC)
    }

    pub fn get_dues<Moment>(&self, now: Moment) -> (Moment, Moment)
    where
        Moment: Parameter + Default + SimpleArithmetic + Copy,
    {
        let due =
            now + Moment::from(TERMS_UNIT) * Moment::from(self.terms) * Moment::from(1000 as u32);
        let due_extend =
            due + Moment::from(DUE_EXTEND) * Moment::from(TERMS_UNIT) * Moment::from(1000 as u32);
        (due, due_extend)
    }
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Loan<AccountId, Balance, Moment> {
    pub id: LoanId,
    pub package_id: LoanPackageId,
    pub who: AccountId,
    pub due: Moment,
    pub due_extend: Moment,
    pub collateral_balance_original: Balance,
    pub collateral_balance_available: Balance,
    pub loan_balance_total: Balance,
    pub status: LoanHealth,
}
impl<AccountId, Balance, Moment> Loan<AccountId, Balance, Moment>
where
    Balance: Encode
        + Decode
        + Parameter
        + Member
        + SimpleArithmetic
        + Default
        + Copy
        + MaybeSerializeDeserialize,
    Moment: Parameter + Default + SimpleArithmetic + Copy,
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + Default,
{
    pub fn get_ltv(collateral_amount: Balance, loan_amount: Balance, btc_price: Price) -> LTV {
        let btc_price_in_balance = <Balance as TryFrom<u128>>::try_from(btc_price)
            .ok()
            .unwrap();
        let ltv = (loan_amount * Balance::from(PRICE_PREC) * Balance::from(LTV_PREC))
            / (collateral_amount * btc_price_in_balance);
        TryInto::<LTV>::try_into(ltv).ok().unwrap()
    }

    pub fn expiration_penalty(&self, penalty_rate: u32) -> Balance {
        self.collateral_balance_available * Balance::from(penalty_rate) / Balance::from(LTV_PREC)
    }

    pub fn expiration_interest<AssetId>(
        &self,
        package: &LoanPackage<Balance, AssetId>,
        btc_price: Price,
    ) -> Balance
    where
        AssetId: Encode + Decode + Parameter + Member + SimpleArithmetic + Default + Copy,
    {
        package.get_interest(self.loan_balance_total) * Balance::from(PRICE_PREC)
            / <Balance as TryFrom<u128>>::try_from(btc_price)
                .ok()
                .unwrap()
    }

    pub fn expire_then_extend<AssetId>(
        &mut self,
        package: &LoanPackage<Balance, AssetId>,
        now: Moment,
        expiration_penalty: Balance,
        expiration_interest: Balance,
    ) where
        AssetId: Encode + Decode + Parameter + Member + SimpleArithmetic + Default + Copy,
    {
        let expiration_fee = expiration_interest + expiration_penalty;
        match self
            .collateral_balance_available
            .checked_sub(&expiration_fee)
        {
            Some(balance) => {
                self.collateral_balance_available = balance;
            }
            None => {
                self.collateral_balance_available = Balance::zero();
            }
        }
        let (due, due_extend) = package.get_dues(now);
        self.due = due;
        self.due_extend = due_extend;
    }
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CreditLine<Balance, AssetId> {
    pub id: CreditLineId,
    pub ltv: LTV,
    pub amount: Balance,
    pub price: Price,
    pub credit: Balance,
    pub asset_id: AssetId,
}

/// The module's configuration trait.
pub trait Trait: assets::Trait + timestamp::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Loan {
        /// the account that user makes loans from, (and assets are all burnt from this account by design)
        PawnShop get(pawn_shop) config() : T::AccountId;
        /// should be exactly the same as in the Saving
        ProfitPool get(profit_pool) config() : T::AccountId;
        /// the asset that user uses as collateral when making loans
        CollateralAssetId get(collateral_asset_id) config() : T::AssetId;
        /// the asset that defi
        LoanAssetId get(loan_asset_id) config() : T::AssetId;
        /// the maximum LTV that a loan package can be set initially
        pub GlobalLTVLimit get(global_ltv_limit) config() : LTV;
        /// when a loan's LTV reaches or is above this threshold, this loan must be been liquidating
        pub GlobalLiquidationThreshold get(global_liquidation_threshold) config() : LTV;
        /// when a loan's LTV reaches or is above this threshold, a warning event will be fired and there should be a centralized system monitoring on this
        pub GlobalWarningThreshold get(global_warning_threshold) config() : LTV;
        /// increase monotonically
        NextLoanPackageId get(next_loan_package_id) config() : LoanPackageId;
        /// currently active packages that users can make new loans with
        pub ActiveLoanPackages get(active_loan_packages) : linked_map LoanPackageId => LoanPackage<T::Balance, T::AssetId>;
        /// all packages including both the active and the inactive
        pub LoanPackages get(loan_package) : linked_map LoanPackageId => LoanPackage<T::Balance, T::AssetId>;
        /// increase monotonically
        NextLoanId get(next_loan_id) config() : LoanId;
        /// currently running loans
        pub Loans get(get_loan_by_id) : linked_map LoanId => Loan<T::AccountId, T::Balance, T::Moment>;
        /// loan id aggregated by account
        pub LoansByAccount get(loans_by_account) : map T::AccountId => Vec<LoanId>;
        /// current btc price coming from Price
        CurrentBTCPrice get(current_btc_price) config() : Price;
        /// total balance of loan asset in circulation
        TotalLoan get(total_loan) : T::Balance;
        /// total balance of collateral asset locked in the pawnshop
        TotalCollateral get(total_collateral) : T::Balance;
        /// total balance of profit that we have gained from fees and penaltys
        TotalProfit get(total_profit) : T::Balance;
        /// when a loan is overdue, a small portion of its collateral will be cut as penalty
        pub PenaltyRate get(penalty_rate) config() : u32;
        /// the official account take charge of selling the collateral asset of liquidating loans
        LiquidationAccount get(liquidation_account) config() : T::AccountId;
        /// loans which are in liquidating, these loans will not be in "Loans" & "LoansByAccount"
        pub LiquidatingLoans get(liquidating_loans) : Vec<LoanId>;
        /// a global cap of loan balance, no caps at all if None
        pub LoanCap get(loan_cap) : Option<T::Balance>;
        /// module level switch
        Paused get(paused) : bool = false;
        /// for each loan, the amount of collateral asset must be greater than this
        pub MinimumCollateral get(minimum_collateral) config() : T::Balance;
        ///
        pub LiquidationPenalty get(liquidation_penalty) config() : u32;
    }
}

// The module's dispatchable functions.
decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const LTV_PRECISION: u32 = LTV_PREC;
        const BTC_PRICE_PRECISION: u32 = PRICE_PREC;
        const INTEREST_RATE_PRECISION: u32 = INTEREST_RATE_PREC;

        fn deposit_event() = default;

        fn on_initialize(height: T::BlockNumber) {
            if !Self::paused() {
                Self::on_each_block(height);
            }
        }

        fn on_finalize(_height: T::BlockNumber) {
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
        pub fn set_collateral_asset_id(origin, asset_id: T::AssetId) -> LoanResult {
            ensure_root(origin)?;
            <CollateralAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_global_ltv_limit(origin, limit: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalLTVLimit::put(limit);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_loan_asset_id(origin, asset_id: T::AssetId) -> LoanResult {
            ensure_root(origin)?;
            <LoanAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_global_liquidation_threshold(origin, threshold: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalWarningThreshold::put(threshold);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_global_warning_threshold(origin, threshold: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalLiquidationThreshold::put(threshold);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_loan_cap(origin, balance: T::Balance) -> LoanResult {
            ensure_root(origin)?;
            if balance.is_zero() {
                <LoanCap<T>>::kill();
            } else {
                <LoanCap<T>>::put(balance);
            }
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_liquidation_account(origin, account_id: T::AccountId) -> LoanResult {
            ensure_root(origin)?;
            <LiquidationAccount<T>>::put(account_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_penalty_rate(origin, rate: u32) -> LoanResult {
            ensure_root(origin)?;
            PenaltyRate::put(rate);
            Ok(())
        }

        /// create a loan package that defines how many days, interest by hour, and minimum TBD about a loan of this package
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn create_package(origin, terms: u32, interest_rate_hourly: u32, min_tbd: T::Balance) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            ensure!(terms > 0 && interest_rate_hourly > 0 && min_tbd > T::Balance::zero(), "zero is not allowed");
            ensure!(interest_rate_hourly < INTEREST_RATE_PREC, "invalid interest rate");
            Self::create_loan_package(terms, interest_rate_hourly, min_tbd)
        }

        /// no more loans can be made from this package, and a disable can't be reversed
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn disable_package(origin, package_id: LoanPackageId) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            Self::disable_loan_package(package_id)
        }

        /// a backdoor to manually set BTC price
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn set_price(origin, price: Price) -> LoanResult {
            ensure_root(origin)?;
            CurrentBTCPrice::put(price);
            Ok(())
        }

        /// a user repay a loan he has made before, by providing the loan id and he should make sure there is enough related assets in his account
        #[weight = SimpleDispatchInfo::FixedNormal(1000_000)]
        pub fn repay(origin, loan_id: LoanId) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            Self::repay_loan(ensure_signed(origin)?, loan_id)
        }

        /// a user can apply for a loan choosing one active loan package, providing the collateral and loan amount he wants,
        #[weight = SimpleDispatchInfo::FixedNormal(1000_000)]
        pub fn apply(origin, collateral_amount: T::Balance, loan_amount: T::Balance, package_id: LoanPackageId) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            Self::apply_for_loan(ensure_signed(origin)?, package_id, collateral_amount, loan_amount)
        }

        /// when a liquidating loan has been handled well, platform mananger should call "mark_liquidated" to update the chain
        /// loan id is the loan been handled and auction_balance is what the liquidation got by selling the collateral asset
        /// auction_balance will be first used to make up the loan, then what so ever left will be returned to the loan's owner account
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn mark_liquidated(origin, loan_id: LoanId, auction_balance: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let liquidation_account = ensure_signed(origin)?;
            ensure!(liquidation_account == Self::liquidation_account(), "liquidation account only");
            ensure!(<Loans<T>>::exists(loan_id), "loan doesn't exists");

            Self::mark_loan_liquidated(&Self::get_loan_by_id(loan_id), liquidation_account, auction_balance)
        }

        /// when user got a warning of high-risk LTV, user can lower the LTV by add more collateral
        #[weight = SimpleDispatchInfo::FixedNormal(1000_000)]
        pub fn add_collateral(origin, loan_id: LoanId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            ensure!(<Loans<T>>::exists(loan_id), "loan doesn't exists");
            let loan = Self::get_loan_by_id(loan_id);
            ensure!(who == loan.who, "adding collateral to other's loan is not allowed");

            Self::add_loan_collateral(&loan, loan.who.clone(), amount)
        }

        /// as long as the LTV of this loan is below the "GlobalLTVLimit", user can keep drawing TBD from this loan
        #[weight = SimpleDispatchInfo::FixedNormal(1000_000)]
        pub fn draw(origin, loan_id: LoanId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            Self::draw_from_loan(who, loan_id, amount)
        }
    }
}

decl_event!(
    #[rustfmt::skip]
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        Balance = <T as pallet_generic_asset::Trait>::Balance,
        Loan = Loan<<T as system::Trait>::AccountId, <T as pallet_generic_asset::Trait>::Balance, <T as timestamp::Trait>::Moment>,
        CollateralBalanceOriginal = <T as pallet_generic_asset::Trait>::Balance,
        CollateralBalanceAvailable = <T as pallet_generic_asset::Trait>::Balance,
        AuctionBalance = <T as pallet_generic_asset::Trait>::Balance,
        TotalLoanBalance = <T as pallet_generic_asset::Trait>::Balance,
    {
        PackageCreated(LoanPackageId),
        PackageDisabled(LoanPackageId),
        LoanCreated(Loan),
        LoanDrawn(LoanId, Balance),
        LoanRepaid(LoanId),
        Expired(LoanId, AccountId),
        Extended(LoanId, AccountId),
        Warning(LoanId, LTV),

        Liquidating(LoanId, AccountId, CollateralBalanceAvailable, TotalLoanBalance),
        Liquidated(
            LoanId,
            CollateralBalanceOriginal,
            CollateralBalanceAvailable,
            AuctionBalance,
            TotalLoanBalance
        ),

        AddCollateral(LoanId, Balance),
    }
);

pub type LoanResult<T = ()> = result::Result<T, &'static str>;

impl<T: Trait> Module<T> {
    pub fn create_loan_package(
        terms: u32,
        interest_rate_hourly: u32,
        min_tbd: T::Balance,
    ) -> DispatchResult {
        let package_id = Self::get_next_loan_package_id();
        let package = LoanPackage {
            id: package_id,
            status: LoanPackageStatus::Active,
            terms: terms,
            min: min_tbd,
            interest_rate_hourly: interest_rate_hourly,
            collateral_asset_id: Self::collateral_asset_id(),
            loan_asset_id: Self::loan_asset_id(),
        };
        <LoanPackages<T>>::insert(package_id, package.clone());
        <ActiveLoanPackages<T>>::insert(package_id, package);

        Self::deposit_event(RawEvent::PackageCreated(package_id));

        Ok(())
    }

    pub fn disable_loan_package(package_id: LoanPackageId) -> DispatchResult {
        ensure!(<LoanPackages<T>>::exists(package_id), "invalid package id");
        <LoanPackages<T>>::mutate(package_id, |v| {
            v.status = LoanPackageStatus::Inactive;
        });

        <ActiveLoanPackages<T>>::remove(package_id);
        Self::deposit_event(RawEvent::PackageDisabled(package_id));
        Ok(())
    }

    pub fn repay_loan(who: T::AccountId, loan_id: LoanId) -> DispatchResult {
        let pawn_shop = Self::pawn_shop();
        ensure!(<Loans<T>>::exists(loan_id), "invalid loan id");
        let loan = <Loans<T>>::get(loan_id);
        ensure!(loan.who == who, "not owner of the loan");
        ensure!(
            <LoanPackages<T>>::exists(loan.package_id),
            "invalid package id in loan"
        );
        let package = Self::loan_package(loan.package_id);
        ensure!(
            <assets::Module<T>>::free_balance(&package.loan_asset_id, &who)
                >= loan.loan_balance_total,
            "not enough asset to repay"
        );
        ensure!(
            <assets::Module<T>>::free_balance(&package.collateral_asset_id, &pawn_shop)
                >= loan.collateral_balance_available,
            "not enough collateral asset in shop"
        );
        ensure!(
            !Self::check_loan_in_liquidation(&loan_id),
            "loan is in liquidation"
        );

        <Loans<T>>::remove(&loan.id);
        <LoansByAccount<T>>::mutate(&who, |v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| *ele != loan_id)
                .collect::<Vec<LoanId>>();
        });
        <TotalLoan<T>>::mutate(|v| *v -= loan.loan_balance_total);
        <TotalCollateral<T>>::mutate(|v| *v -= loan.collateral_balance_available);
        let revert_callback = || {
            <Loans<T>>::insert(&loan.id, &loan);
            <LoansByAccount<T>>::mutate(&who, |v| {
                v.push(loan.id);
            });
            <TotalLoan<T>>::mutate(|v| *v += loan.loan_balance_total);
            <TotalCollateral<T>>::mutate(|v| *v += loan.collateral_balance_available);
        };

        <assets::Module<T>>::make_transfer_with_event(
            &package.loan_asset_id,
            &who,
            &pawn_shop,
            loan.loan_balance_total,
        )
        .or_else(|err| -> DispatchResult {
            revert_callback();
            Err(err)
        })?;
        <assets::Module<T>>::make_transfer_with_event(
            &package.collateral_asset_id,
            &pawn_shop,
            &who,
            loan.collateral_balance_available,
        )
        .or_else(|err| -> DispatchResult {
            revert_callback();
            <assets::Module<T>>::make_transfer_with_event(
                &package.loan_asset_id,
                &pawn_shop,
                &who,
                loan.loan_balance_total,
            )?;
            Err(err)
        })?;
        <assets::Module<T>>::burn(
            system::RawOrigin::Root.into(),
            package.loan_asset_id.clone(),
            pawn_shop.clone(),
            loan.loan_balance_total,
        )?;

        Self::deposit_event(RawEvent::LoanRepaid(loan_id));
        Ok(())
    }

    pub fn draw_from_loan(
        who: T::AccountId,
        loan_id: LoanId,
        amount: T::Balance,
    ) -> DispatchResult {
        ensure!(<Loans<T>>::exists(loan_id), "invalid loan id");
        let loan = Self::get_loan_by_id(loan_id);
        ensure!(loan.who == who, "can't draw from others loan");
        let btc_price = Self::current_btc_price();
        let global_ltv = Self::global_ltv_limit();
        let available_credit = loan.collateral_balance_available
            * <T::Balance as TryFrom<u128>>::try_from(btc_price)
                .ok()
                .unwrap()
            * <T::Balance as TryFrom<u64>>::try_from(global_ltv)
                .ok()
                .unwrap()
            / T::Balance::from(LTV_PREC)
            / T::Balance::from(PRICE_PREC)
            - loan.loan_balance_total;
        ensure!(amount <= available_credit, "short of available credit");

        let profit_pool = Self::profit_pool();
        let package = Self::loan_package(loan.package_id);
        let interest = package.get_interest(amount);

        <assets::Module<T>>::mint(
            system::RawOrigin::Root.into(),
            package.loan_asset_id.clone(),
            profit_pool.clone(),
            interest,
        )?;

        <assets::Module<T>>::mint(
            system::RawOrigin::Root.into(),
            package.loan_asset_id.clone(),
            who.clone(),
            amount - interest,
        )
        .or_else(|err| -> DispatchResult {
            <assets::Module<T>>::burn(
                system::RawOrigin::Root.into(),
                package.loan_asset_id.clone(),
                profit_pool.clone(),
                interest,
            )?;
            Err(err)
        })?;

        <Loans<T>>::mutate(loan_id, |v| {
            v.loan_balance_total += amount;
        });
        <TotalLoan<T>>::mutate(|v| *v += amount);
        <TotalProfit<T>>::mutate(|v| *v += interest);

        Self::deposit_event(RawEvent::LoanDrawn(loan_id, amount));

        Ok(())
    }

    pub fn apply_for_loan(
        who: T::AccountId,
        package_id: LoanPackageId,
        collateral_amount: T::Balance,
        loan_amount: T::Balance,
    ) -> DispatchResult {
        ensure!(
            !(collateral_amount.is_zero() && loan_amount.is_zero()),
            "invalid combination of collateral & loan amount"
        );
        ensure!(
            <ActiveLoanPackages<T>>::exists(package_id),
            "invalid package id"
        );
        let package = <ActiveLoanPackages<T>>::get(package_id);
        let shop = <PawnShop<T>>::get();
        let profit_pool = Self::profit_pool();
        let loan_cap = <LoanCap<T>>::get();
        let total_loan = <TotalLoan<T>>::get();

        if loan_cap.is_some() && total_loan >= loan_cap.unwrap() {
            return Err("reach loan limit");
        }

        match Self::get_collateral_loan(collateral_amount, loan_amount) {
            Err(err) => Err(err),
            Ok(CollateralLoan {
                collateral_amount: actual_collateral_amount,
                loan_amount: actual_loan_amount,
            }) => {
                ensure!(
                    package.min <= actual_loan_amount,
                    "not reach min loan amount"
                );
                ensure!(
                    collateral_amount >= Self::minimum_collateral(),
                    "not reach min collateral amount"
                );

                let interest = package.get_interest(actual_loan_amount);
                ensure!(interest < actual_loan_amount, "interest is too high");

                <assets::Module<T>>::make_transfer_with_event(
                    &package.collateral_asset_id,
                    &who,
                    &shop,
                    actual_collateral_amount,
                )?;

                let now = <timestamp::Module<T>>::get();
                let (due, due_extend) = package.get_dues(now);
                let loan_id = Self::get_next_loan_id();
                let loan = Loan {
                    id: loan_id,
                    package_id: package_id,
                    who: who.clone(),
                    due: due,
                    due_extend: due_extend,
                    collateral_balance_original: actual_collateral_amount,
                    collateral_balance_available: actual_collateral_amount,
                    loan_balance_total: actual_loan_amount,
                    status: Default::default(),
                };

                <assets::Module<T>>::mint(
                    system::RawOrigin::Root.into(),
                    package.loan_asset_id.clone(),
                    profit_pool.clone(),
                    interest,
                )
                .or_else(|err| -> DispatchResult {
                    <assets::Module<T>>::make_transfer_with_event(
                        &package.collateral_asset_id,
                        &shop,
                        &who,
                        actual_collateral_amount,
                    )?;
                    Err(err)
                })?;

                <assets::Module<T>>::mint(
                    system::RawOrigin::Root.into(),
                    package.loan_asset_id.clone(),
                    who.clone(),
                    actual_loan_amount - interest,
                )
                .or_else(|err| {
                    <assets::Module<T>>::burn(
                        system::RawOrigin::Root.into(),
                        package.loan_asset_id.clone(),
                        profit_pool.clone(),
                        interest,
                    )
                    .and_then(|()| -> DispatchResult {
                        <assets::Module<T>>::make_transfer_with_event(
                            &package.collateral_asset_id,
                            &shop,
                            &who,
                            actual_collateral_amount,
                        )
                    })?;
                    Err(err)
                })?;

                <Loans<T>>::insert(loan_id, loan.clone());
                <LoansByAccount<T>>::mutate(&who, |v| {
                    v.push(loan_id);
                });
                <TotalLoan<T>>::mutate(|v| *v += actual_loan_amount);
                <TotalCollateral<T>>::mutate(|v| *v += actual_collateral_amount);
                <TotalProfit<T>>::mutate(|v| *v += interest);

                Self::deposit_event(RawEvent::LoanCreated(loan));
                Ok(())
            }
        }
    }

    pub fn mark_loan_liquidated(
        loan: &Loan<T::AccountId, T::Balance, T::Moment>,
        liquidation_account: T::AccountId,
        auction_balance: T::Balance,
    ) -> DispatchResult {
        ensure!(
            Self::check_loan_in_liquidation(&loan.id),
            "loan id not in liquidating"
        );
        let pawnshop = Self::pawn_shop();
        let package = Self::loan_package(loan.package_id);
        ensure!(
            <assets::Module<T>>::free_balance(&package.loan_asset_id, &liquidation_account)
                >= auction_balance,
            "not enough asset to liquidate"
        );
        <assets::Module<T>>::make_transfer_with_event(
            &package.loan_asset_id,
            &liquidation_account,
            &pawnshop,
            loan.loan_balance_total,
        )?;
        let leftover = auction_balance.checked_sub(&loan.loan_balance_total);
        if leftover.is_some() && leftover.unwrap() > T::Balance::zero() {
            let penalty_rate = Self::liquidation_penalty();
            let penalty =
                leftover.unwrap() * T::Balance::from(penalty_rate) / T::Balance::from(LTV_PREC);
            <assets::Module<T>>::make_transfer_with_event(
                &package.loan_asset_id,
                &liquidation_account,
                &Self::profit_pool(),
                penalty,
            )
            .or_else(|err| -> DispatchResult {
                <assets::Module<T>>::make_transfer_with_event(
                    &package.loan_asset_id,
                    &pawnshop,
                    &liquidation_account,
                    loan.loan_balance_total,
                )?;
                Err(err)
            })?;
            <assets::Module<T>>::make_transfer_with_event(
                &package.loan_asset_id,
                &liquidation_account,
                &loan.who,
                leftover.unwrap() - penalty,
            )
            .or_else(|err| -> DispatchResult {
                // revert previous transfer
                <assets::Module<T>>::make_transfer_with_event(
                    &package.loan_asset_id,
                    &Self::profit_pool(),
                    &liquidation_account,
                    penalty,
                )?;
                <assets::Module<T>>::make_transfer_with_event(
                    &package.loan_asset_id,
                    &pawnshop,
                    &liquidation_account,
                    loan.loan_balance_total,
                )?;
                Err(err)
            })?;
        }
        <Loans<T>>::remove(&loan.id);
        <LoansByAccount<T>>::mutate(&loan.who, |v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| ele != &loan.id)
                .collect::<Vec<LoanId>>();
        });
        LiquidatingLoans::mutate(|v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| ele != &loan.id)
                .collect::<Vec<LoanId>>();
        });
        Self::deposit_event(RawEvent::Liquidated(
            loan.id,
            loan.collateral_balance_original,
            loan.collateral_balance_available,
            auction_balance,
            loan.loan_balance_total,
        ));

        Ok(())
    }

    pub fn add_loan_collateral(
        loan: &Loan<T::AccountId, T::Balance, T::Moment>,
        from: T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        let pawnshop = Self::pawn_shop();
        let package = Self::loan_package(loan.package_id);

        ensure!(
            <assets::Module<T>>::free_balance(&package.collateral_asset_id, &from) >= amount,
            "not enough collateral asset in free balance"
        );

        <assets::Module<T>>::make_transfer_with_event(
            &package.collateral_asset_id,
            &from,
            &pawnshop,
            amount,
        )?;

        <Loans<T>>::mutate(loan.id, |l| {
            l.collateral_balance_original += amount;
            l.collateral_balance_available += amount;
        });

        <TotalCollateral<T>>::mutate(|c| {
            *c += amount;
        });

        Self::deposit_event(RawEvent::AddCollateral(loan.id, amount));

        Ok(())
    }

    pub fn get_collateral_loan(
        collateral_amount: T::Balance,
        loan_amount: T::Balance,
    ) -> Result<CollateralLoan<T::Balance>, &'static str> {
        if collateral_amount.is_zero() && loan_amount.is_zero() {
            return Err("both amount are zero");
        }

        let btc_price = CurrentBTCPrice::get();
        let ltv = GlobalLTVLimit::get();
        let btc_price_in_balance = <T::Balance as TryFrom<u128>>::try_from(btc_price)
            .ok()
            .unwrap();
        let price_prec_in_balance = T::Balance::from(PRICE_PREC);
        let ltv_prec_in_balance = T::Balance::from(LTV_PREC);
        let ltv_in_balance = <T::Balance as TryFrom<u64>>::try_from(ltv).ok().unwrap();

        if collateral_amount.is_zero() {
            let must_collateral_amount = loan_amount * ltv_prec_in_balance * price_prec_in_balance
                / (btc_price_in_balance * ltv_in_balance);
            return Ok(CollateralLoan {
                collateral_amount: must_collateral_amount,
                loan_amount: loan_amount,
            });
        }

        if loan_amount.is_zero() {
            let can_loan_amount = (collateral_amount * btc_price_in_balance * ltv_in_balance)
                / (ltv_prec_in_balance * price_prec_in_balance);
            return Ok(CollateralLoan {
                collateral_amount: collateral_amount,
                loan_amount: can_loan_amount,
            });
        }

        if (loan_amount * ltv_prec_in_balance) * price_prec_in_balance
            / collateral_amount
            / btc_price_in_balance
            > ltv_in_balance
        {
            Err("over LTV limit")
        } else {
            Ok(CollateralLoan {
                collateral_amount,
                loan_amount,
            })
        }
    }

    fn get_next_loan_package_id() -> LoanPackageId {
        NextLoanPackageId::mutate(|v| {
            let org = *v;
            *v += 1;
            org
        })
    }

    fn get_next_loan_id() -> LoanId {
        NextLoanId::mutate(|v| {
            let org = *v;
            *v += 1;
            org
        })
    }

    fn on_each_block(_height: T::BlockNumber) {
        let now = <timestamp::Module<T>>::get();
        let btc_price = Self::current_btc_price();
        let liquidation_thd = Self::global_liquidation_threshold();
        let warning_thd = Self::global_warning_threshold();
        let mut packages = btree_map::BTreeMap::new();
        let mut total_penalty = T::Balance::zero();
        let mut total_interest = T::Balance::zero();
        let collateral_asset_id = Self::collateral_asset_id();
        let pawnshop = Self::pawn_shop();
        let profit_pool = Self::profit_pool();

        for (loan_id, loan) in <Loans<T>>::enumerate() {
            if Self::check_loan_in_liquidation(&loan_id) {
                continue;
            }

            match Self::check_loan_health(&loan, now, btc_price, liquidation_thd, warning_thd) {
                LoanHealth::Well => {}
                LoanHealth::Warning(ltv) => {
                    <Loans<T>>::mutate(&loan.id, |v| v.status = LoanHealth::Warning(ltv));
                    Self::deposit_event(RawEvent::Warning(loan_id, ltv));
                }
                LoanHealth::Extended => {
                    <Loans<T>>::mutate(&loan.id, |v| v.status = LoanHealth::Extended);
                    Self::deposit_event(RawEvent::Extended(loan_id, loan.who));
                }
                LoanHealth::Liquidating(l) => {
                    Self::liquidate_loan(loan_id, l);
                    Self::deposit_event(RawEvent::Liquidating(
                        loan_id,
                        loan.who.clone(),
                        loan.collateral_balance_available,
                        loan.loan_balance_total,
                    ));
                }
                LoanHealth::Expired => {
                    if !packages.contains_key(&loan.package_id) {
                        packages.insert(loan.package_id, <LoanPackages<T>>::get(loan.package_id));
                    }
                    let package = packages.get(&loan.package_id).unwrap();
                    let penalty = loan.expiration_penalty(Self::penalty_rate());
                    let interest = loan.expiration_interest(package, btc_price);

                    total_penalty += penalty;
                    total_interest += interest;

                    let who = loan.who.clone();
                    let mut new_loan = loan.clone();
                    new_loan.expire_then_extend(package, now, penalty, interest);
                    let new_ltv = <Loan<T::AccountId, T::Balance, T::Moment>>::get_ltv(
                        new_loan.collateral_balance_available,
                        new_loan.loan_balance_total,
                        btc_price,
                    );

                    if new_ltv >= liquidation_thd {
                        Self::liquidate_loan(loan_id, new_ltv);
                        Self::deposit_event(RawEvent::Liquidating(
                            loan_id,
                            loan.who.clone(),
                            loan.collateral_balance_available,
                            loan.loan_balance_total,
                        ));
                    } else if new_ltv >= warning_thd {
                        Self::deposit_event(RawEvent::Warning(loan_id, new_ltv));
                    } else {
                        <Loans<T>>::insert(loan_id, new_loan);
                        Self::deposit_event(RawEvent::Expired(loan_id, who));
                    }
                }
            }
        }

        if !(total_penalty + total_interest).is_zero() {
            <assets::Module<T>>::make_transfer_with_event(
                &collateral_asset_id,
                &pawnshop,
                &profit_pool,
                total_penalty + total_interest,
            )
            .and_then(|_| {
                <TotalCollateral<T>>::mutate(|v| {
                    match v.checked_sub(&(total_interest + total_penalty)) {
                        Some(total) => {
                            *v = total;
                            Ok(())
                        }
                        None => Err("total collateral underflows"),
                    }
                })
            })
            .unwrap_or_default();
        }
    }

    fn liquidate_loan(loan_id: LoanId, liquidating_ltv: LTV) {
        <Loans<T>>::mutate(loan_id, |v| {
            v.status = LoanHealth::Liquidating(liquidating_ltv)
        });
        if LiquidatingLoans::exists() {
            LiquidatingLoans::mutate(|v| v.push(loan_id));
        } else {
            let ll: Vec<LoanId> = vec![loan_id];
            LiquidatingLoans::put(ll);
        }
    }

    fn check_loan_in_liquidation(loan_id: &LoanId) -> bool {
        LiquidatingLoans::get().contains(loan_id)
    }

    fn check_loan_health(
        loan: &Loan<T::AccountId, T::Balance, T::Moment>,
        now: T::Moment,
        btc_price: Price,
        liquidation: LTV,
        warning: LTV,
    ) -> LoanHealth {
        let current_ltv = <Loan<T::AccountId, T::Balance, T::Moment>>::get_ltv(
            loan.collateral_balance_available,
            loan.loan_balance_total,
            btc_price,
        );

        if current_ltv >= liquidation {
            return LoanHealth::Liquidating(current_ltv);
        }

        if current_ltv >= warning {
            return LoanHealth::Warning(current_ltv);
        }

        if loan.due_extend <= now {
            return LoanHealth::Expired;
        }

        if loan.due <= now {
            return LoanHealth::Extended;
        }

        LoanHealth::Well
    }
}

impl<T: Trait> assets::traits::OnAssetTransfer<T::AssetId, T::AccountId, T::Balance> for Module<T> {
    fn on_asset_transfer(
        _asset_id: &T::AssetId,
        _from: &T::AccountId,
        _to: &T::AccountId,
        _balance: &T::Balance,
    ) -> DispatchResult {
        Ok(())
    }
}

/// implement the price::OnChange hook to be aware of the price changes
impl<T: Trait> price::OnChange for Module<T> {
    fn on_change(p: price::Price) {
        CurrentBTCPrice::put(p);
    }
}
