#![cfg_attr(not(feature = "std"), no_std)]

/// This module extends the pallet-generic-asset module.
/// With an extra asset symbol for each asset.
/// Currently we are using a very simple management model, the pallet-sudo,
/// the more decentralized strategy is incoming.
mod mock;
mod tests;

use rstd::{result, vec::Vec};
use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, weights::SimpleDispatchInfo,
};
use system::{ensure_root, ensure_signed, Error};

use sp_runtime::traits::Dispatchable;

pub use generic_asset::AssetOptions;
pub use generic_asset::PermissionLatest;

pub mod traits {
    use super::Result;

    pub trait BeforeAssetCreate<AssetId> {
        fn before_asset_create(_asset_id: &AssetId) -> Result {
            Ok(())
        }
    }
    pub trait OnAssetCreate<AssetId> {
        fn on_asset_create(_asset_id: &AssetId) -> Result {
            Ok(())
        }
    }

    pub trait BeforeAssetTransfer<AssetId, AccountId, Balance> {
        fn before_asset_transfer(
            _asset_id: &AssetId,
            _from: &AccountId,
            _to: &AccountId,
            _balance: &Balance,
        ) -> Result {
            Ok(())
        }
    }

    pub trait OnAssetTransfer<AssetId, AccountId, Balance> {
        fn on_asset_transfer(
            _asset_id: &AssetId,
            _from: &AccountId,
            _to: &AccountId,
            _balance: &Balance,
        ) -> Result {
            Ok(())
        }
    }

    pub trait BeforeAssetMint<AssetId, AccountId, Balance> {
        fn before_asset_mint(_asset_id: &AssetId, _to: &AccountId, _balance: &Balance) -> Result {
            Ok(())
        }
    }
    pub trait OnAssetMint<AssetId, AccountId, Balance> {
        fn on_asset_mint(_asset_id: &AssetId, _to: &AccountId, _balance: &Balance) -> Result {
            Ok(())
        }
    }

    pub trait BeforeAssetBurn<AssetId, AccountId, Balance> {
        fn before_asset_burn(_asset_id: &AssetId, _to: &AccountId, _balance: &Balance) -> Result {
            Ok(())
        }
    }
    pub trait OnAssetBurn<AssetId, AccountId, Balance> {
        fn on_asset_burn(_asset_id: &AssetId, _to: &AccountId, _balance: &Balance) -> Result {
            Ok(())
        }
    }

    impl<A> BeforeAssetCreate<A> for () {}
    impl<A> OnAssetCreate<A> for () {}
    impl<A, B, C> BeforeAssetBurn<A, B, C> for () {}
    impl<A, B, C> OnAssetBurn<A, B, C> for () {}
    impl<A, B, C> BeforeAssetMint<A, B, C> for () {}
    impl<A, B, C> OnAssetMint<A, B, C> for () {}
    impl<A, B, C> BeforeAssetTransfer<A, B, C> for () {}
    impl<A, B, C> OnAssetTransfer<A, B, C> for () {}
}
use crate::traits::*;

/// The module's configuration trait.
pub trait Trait: generic_asset::Trait + sudo::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    type BeforeAssetTransfer: crate::traits::BeforeAssetTransfer<
        Self::AssetId,
        Self::AccountId,
        Self::Balance,
    >;
    type BeforeAssetCreate: crate::traits::BeforeAssetCreate<Self::AssetId>;
    type BeforeAssetMint: crate::traits::BeforeAssetMint<
        Self::AssetId,
        Self::AccountId,
        Self::Balance,
    >;
    type BeforeAssetBurn: crate::traits::BeforeAssetBurn<
        Self::AssetId,
        Self::AccountId,
        Self::Balance,
    >;

    type OnAssetTransfer: crate::traits::OnAssetTransfer<
        Self::AssetId,
        Self::AccountId,
        Self::Balance,
    >;
    type OnAssetCreate: crate::traits::OnAssetCreate<Self::AssetId>;
    type OnAssetMint: crate::traits::OnAssetMint<Self::AssetId, Self::AccountId, Self::Balance>;
    type OnAssetBurn: crate::traits::OnAssetBurn<Self::AssetId, Self::AccountId, Self::Balance>;
}

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Assets {
        /// "Symbols" can only keep Vec<u8>, and utf8 safty is totally on the client side
        pub Symbols get(symbols) config() : map T::AssetId => Vec<u8>;
    }

    add_extra_genesis {
        build(|config: &GenesisConfig<T>| {
            let origin = <sudo::Module<T>>::key();
            let options = AssetOptions {
                initial_issuance: T::Balance::from(0),
                permissions: PermissionLatest {
                    update: generic_asset::Owner::Address(origin.clone()),
                    mint: generic_asset::Owner::Address(origin.clone()),
                    burn: generic_asset::Owner::Address(origin.clone()),
                },
            };
            for i in &config.symbols {
                <generic_asset::Module<T>>::create_asset(Some(i.0), None, options.clone()).unwrap();
            }
        });
    }
}

// The module's dispatchable functions.
decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your module
        pub fn deposit_event() = default;

        /// create a new asset with full permissions granted to whoever make the call
        /// *sudo or proposal approved only*
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn create(origin, initial_balance: T::Balance, symbol: Vec<u8>) -> Result {
            ensure_root(origin)?;
            let root_account_id = <sudo::Module<T>>::key();

            let options = AssetOptions {
                initial_issuance:initial_balance,
                permissions: PermissionLatest {
                    update: generic_asset::Owner::Address(root_account_id.clone()),
                    mint: generic_asset::Owner::Address(root_account_id.clone()),
                    burn: generic_asset::Owner::Address(root_account_id.clone()),
                },
            };

            let asset_id = <generic_asset::Module<T>>::next_asset_id();
            <generic_asset::Module<T>>::create_asset(None, Some(root_account_id), options)?;
            <Symbols<T>>::insert(asset_id, symbol.clone());

            Ok(())
        }

        /// generic_asset<T>::make_transfer_with_event delegation
        /// wrap 2 hooks around "make_transfer_with_event": T::BeforeAssetTransfer & T::OnAssetTransfer
        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn transfer(origin, #[compact] asset_id: T::AssetId, to: T::AccountId, #[compact] amount: T::Balance) -> Result {
            let o = ensure_signed(origin)?;
            T::BeforeAssetTransfer::before_asset_transfer(&asset_id, &o, &to, &amount)?;
            <generic_asset::Module<T>>::make_transfer_with_event(&asset_id, &o, &to, amount)?;
            // ignore the err
            T::OnAssetTransfer::on_asset_transfer(&asset_id, &o, &to, &amount).unwrap_or_default();
            Ok(())
            // generic_asset::Call::<T>::transfer(asset_id, to, amount).dispatch(origin)
        }

        // generic_asset<T>::update_permission delegation
        // pub fn update_permission(origin, #[compact] asset_id: T::AssetId, new_permission: PermissionLatest<T::AccountId>) -> Result {
        //     generic_asset::Call::<T>::update_permission(asset_id, new_permission).dispatch(origin)
        // }

        /// generic_asset<T>::mint delegation
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn mint(origin, #[compact] asset_id: T::AssetId, to: T::AccountId, amount: T::Balance) -> Result {
            ensure_root(origin)?;
            let root_account_id = <sudo::Module<T>>::key();
            T::BeforeAssetMint::before_asset_mint(&asset_id, &to, &amount)?;
            generic_asset::Call::<T>::mint(asset_id.clone(), to.clone(), amount).dispatch(system::RawOrigin::Signed(root_account_id).into())?;
            // ignore the err
            T::OnAssetMint::on_asset_mint(&asset_id, &to, &amount).unwrap_or_default();
            Ok(())
        }

        /// generic_asset<T>::burn delegation
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn burn(origin, #[compact] asset_id: T::AssetId, to: T::AccountId, amount:T::Balance) -> Result {
            ensure_root(origin)?;
            let root_account_id = <sudo::Module<T>>::key();
            T::BeforeAssetBurn::before_asset_burn(&asset_id, &to, &amount)?;
            generic_asset::Call::<T>::burn(asset_id.clone(), to.clone(), amount).dispatch(system::RawOrigin::Signed(root_account_id).into())?;
            // ignore the err
            T::OnAssetBurn::on_asset_burn(&asset_id, &to, &amount).unwrap_or_default();
            Ok(())
        }

        /// generic_asset<T>::create_reserved delegation
        #[weight = SimpleDispatchInfo::FreeOperational]
        pub fn create_reserved(origin, asset_id: T::AssetId, options: AssetOptions<T::Balance, T::AccountId>) -> Result {
            ensure_root(origin)?;
            let root_account_id = <sudo::Module<T>>::key();
            generic_asset::Call::<T>::create_reserved(asset_id, options).dispatch(system::RawOrigin::Signed(root_account_id).into())
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn ensure_signed_root(
        origin: <T as system::Trait>::Origin,
    ) -> result::Result<<T as system::Trait>::AccountId, &'static str> {
        let who = ensure_signed(origin)?;
        ensure!(
            who == <sudo::Module<T>>::key(),
            Error::RequireRootOrigin.into()
        );
        Ok(who)
    }

    pub fn get_current_asset_id() -> T::AssetId {
        <generic_asset::Module<T>>::next_asset_id()
    }

    pub fn asset_exists(asset_id: &T::AssetId) -> bool {
        <Symbols<T>>::exists(asset_id)
    }

    pub fn free_balance(asset_id: &T::AssetId, who: &T::AccountId) -> T::Balance {
        <generic_asset::Module<T>>::free_balance(asset_id, who)
    }

    pub fn make_transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: T::Balance,
    ) -> Result {
        <generic_asset::Module<T>>::make_transfer(asset_id, from, to, amount)
    }

    pub fn make_transfer_with_event(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: T::Balance,
    ) -> Result {
        <generic_asset::Module<T>>::make_transfer_with_event(asset_id, from, to, amount)
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        // Just a dummy event.
        // Event `Something` is declared with a parameter of the type `u32` and `AccountId`
        // To emit this event, we call the deposit funtion, from our runtime funtions
        PhantomEvent(u32, AccountId),
    }
);
