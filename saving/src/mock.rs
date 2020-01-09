#![cfg(test)]
#![allow(dead_code)]

use primitives::H256;
use support::{
    construct_runtime, impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types,
    weights::Weight,
};
// The testing primitives are very useful for avoiding having to work with signatures
// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.
use crate::{Module, Trait};
use assets;
use balances;
use std::cell::RefCell;

#[allow(unused_imports)]
use sp_runtime::{
    testing::Header,
    traits::{
        BlakeTwo256, Block as BlockT, ConvertInto, IdentityLookup, OnFinalize, OnInitialize,
        StaticLookup,
    },
    MultiSignature, Perbill,
};

use primitives::u32_trait::*;

use crate as saving;

thread_local! {
    pub(crate) static EXISTENTIAL_DEPOSIT: RefCell<u128> = RefCell::new(0);
    static TRANSFER_FEE: RefCell<u128> = RefCell::new(0);
    static CREATION_FEE: RefCell<u128> = RefCell::new(0);
}

pub mod constants {
    use super::TestRuntime;

    pub const DECIMALS: u128 = 100000000; // satoshi

    pub const ROOT: <TestRuntime as system::Trait>::AccountId = 1;
    pub const ALICE: <TestRuntime as system::Trait>::AccountId = 2;
    pub const BOB: <TestRuntime as system::Trait>::AccountId = 3;
    pub const CHRIS: <TestRuntime as system::Trait>::AccountId = 4;
    #[allow(dead_code)]
    pub const DAVE: <TestRuntime as system::Trait>::AccountId = 5;
    pub const TEAM: <TestRuntime as system::Trait>::AccountId = 6;
    pub const PROFIT_POOL: <TestRuntime as system::Trait>::AccountId = 7;

    pub const COLLECTION_ACCOUNT_ID: <TestRuntime as system::Trait>::AccountId = 999;

    pub const NUM_OF_PHASE: u32 = 5;
    pub const PHASE1: u32 = 1;
    pub const PHASE2: u32 = 2;
    pub const PHASE3: u32 = 3;
    pub const PHASE4: u32 = 4;
    pub const PHASE5: u32 = 5;
    pub const PHASE1_QUOTA: u128 = 100_00000000;
    pub const PHASE2_QUOTA: u128 = 400_00000000;
    pub const PHASE3_QUOTA: u128 = 1000_00000000;
    pub const PHASE4_QUOTA: u128 = 5000_00000000;
    pub const PHASE5_QUOTA: u128 = 100000_00000000;
    pub const PHASE1_EXCHANGE: u128 = 10000;
    pub const PHASE2_EXCHANGE: u128 = 8000;
    pub const PHASE3_EXCHANGE: u128 = 5000;
    pub const PHASE4_EXCHANGE: u128 = 2000;
    pub const PHASE5_EXCHANGE: u128 = 1000;

    pub const RBTC_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC1_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC2_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC3_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC4_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC5_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const SBTC_INITIAL_BALANCE: u128 = 0 * DECIMALS;

    pub const RBTC_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 1;
    pub const RSC1_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 2;
    pub const RSC2_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 3;
    pub const RSC3_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 4;
    pub const RSC4_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 5;
    pub const RSC5_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 6;
    pub const SBTC_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 7;
    pub const TBD_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 8;
}

use self::constants::*;

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
// #[derive(Clone, Eq, PartialEq)]
// pub struct TestRuntime;

// impl_outer_origin! {
//     pub enum Origin for TestRuntime {}
// }

// mod saving {
//     pub use crate::Event;
// }
// impl_outer_event! {
//     pub enum TestEvent for TestRuntime {
//         saving<T>,
//     }
// }
// type Balances = balances::Module<TestRuntime>;
// type System = system::Module<TestRuntime>;
// type Sudo = sudo::Module<TestRuntime>;
// type Assets = assets::Module<TestRuntime>;
// impl_outer_dispatch! {
//     pub enum Call for TestRuntime where origin: Origin {
//         balances::Balances,
//         system::System,
//         sudo::Sudo,
//         assets::Assets,
//     }
// }

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: u32 = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Trait for TestRuntime {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Call = ();
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
}
parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
}
impl balances::Trait for TestRuntime {
    type Balance = u128;
    type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type Event = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
}

pub type Address = <Indices as StaticLookup>::Source;
pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u128;
pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic =
    sp_runtime::generic::UncheckedExtrinsic<Address, Call, MultiSignature, ()>;

construct_runtime!(
		pub enum TestRuntime where
			  Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
		{
			  System: system::{Module, Call, Event},
        Balances: balances,
        OracleMembers: collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>},
        Sudo: sudo,
        Assets: assets::{Module, Call, Storage, Config<T>, Event<T>},
        Oracle: oracle::{Module, Call, Storage, Event<T>},
        SavingTest: saving::{Module, Call, Storage, Config<T>, Event<T>},
		    Indices: indices,
		}
);

impl indices::Trait for TestRuntime {
    type AccountIndex = u32;
    type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    type IsDeadAccount = Balances;
    type Event = ();
}

type OracleCollective = collective::Instance1;

impl collective::Trait<OracleCollective> for TestRuntime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = ();
}

parameter_types! {
    pub const PricePrecision: u32 = price::PRICE_PRECISION;
}

impl price::Trait for TestRuntime {
    type Event = ();
    type OracleMixedIn = Oracle;
    type ReportOrigin = collective::EnsureMember<AccountId, OracleCollective>;
    type OnChange = ();
}

parameter_types! {
    pub const OracleFee: Balance = 1;
    pub const MissReportSlash: Balance = 1;
    pub const MinStaking: Balance = 1000;
    pub const Count: u16 = 3;
    pub const ReportInteval: BlockNumber = 10;
    pub const ElectionEra: BlockNumber = 10;
    pub const LockedDuration: BlockNumber = 1000;
}

impl oracle::Trait for TestRuntime {
    type Event = ();
    type Currency = Balances;
    type OracleFee = OracleFee;
    type MissReportSlash = MissReportSlash;
    type MinStaking = MinStaking;
    type MaliciousSlashOrigin =
        collective::EnsureProportionMoreThan<_1, _2, AccountId, OracleCollective>;
    type Count = Count;
    type ReportInteval = ReportInteval;
    type ElectionEra = ElectionEra;
    type LockedDuration = LockedDuration;
    type ChangeMembers = OracleMembers;
}
// parameter_types! {
//       pub const TransactionBaseFee: u128 = 0;
//       pub const TransactionByteFee: u128 = 1;
// }
// impl transaction_payment::Trait for TestRuntime {
//     type Currency = Module<TestRuntime>;
//     type OnTransactionPayment = ();
//     type TransactionBaseFee = TransactionBaseFee;
//     type TransactionByteFee = TransactionByteFee;
//     type WeightToFee = ConvertInto;
//     type FeeMultiplierUpdate = ();
// }
parameter_types! {
    pub const MinimumPeriod: u64 = 1000;
}
impl timestamp::Trait for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
}
impl sudo::Trait for TestRuntime {
    type Event = ();
    type Proposal = Call;
}

impl pallet_generic_asset::Trait for TestRuntime {
    type Event = ();
    type Balance = u128;
    type AssetId = u32;
}
impl assets::Trait for TestRuntime {
    type Event = ();
    type OnAssetMint = SavingTest;
    type OnAssetCreate = ();
    type OnAssetTransfer = SavingTest;
    type OnAssetBurn = SavingTest;
    type BeforeAssetMint = ();
    type BeforeAssetCreate = ();
    type BeforeAssetTransfer = SavingTest;
    type BeforeAssetBurn = ();
}
impl Trait for TestRuntime {
    type Event = ();
}

pub struct ExtBuilder {}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl ExtBuilder {
    pub fn build(self) -> runtime_io::TestExternalities {
        new_test_ext()
    }
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> runtime_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();

    sudo::GenesisConfig::<TestRuntime> { key: ROOT }
        .assimilate_storage(&mut t)
        .unwrap();

    pallet_generic_asset::GenesisConfig::<TestRuntime> {
        next_asset_id: 9,
        staking_asset_id: 0,
        spending_asset_id: 0,
        assets: vec![],
        initial_balance: 0,
        endowed_accounts: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    assets::GenesisConfig::<TestRuntime> {
        symbols: vec![
            (SBTC_ASSET_ID, "SBTC".as_bytes().to_vec()),
            (RBTC_ASSET_ID, "RBTC".as_bytes().to_vec()),
            (RSC1_ASSET_ID, "RSC1".as_bytes().to_vec()),
            (RSC2_ASSET_ID, "RSC2".as_bytes().to_vec()),
            (RSC3_ASSET_ID, "RSC3".as_bytes().to_vec()),
            (RSC4_ASSET_ID, "RSC4".as_bytes().to_vec()),
            (RSC5_ASSET_ID, "RSC5".as_bytes().to_vec()),
            (TBD_ASSET_ID, "TBD".as_bytes().to_vec()),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    saving::GenesisConfig::<TestRuntime> {
        current_phase_id: 1,
        phase_infos: vec![
            (PHASE1_QUOTA, PHASE1_EXCHANGE, RSC1_ASSET_ID),
            (PHASE2_QUOTA, PHASE2_EXCHANGE, RSC2_ASSET_ID),
            (PHASE3_QUOTA, PHASE3_EXCHANGE, RSC3_ASSET_ID),
            (PHASE4_QUOTA, PHASE4_EXCHANGE, RSC4_ASSET_ID),
            (PHASE5_QUOTA, PHASE5_EXCHANGE, RSC5_ASSET_ID),
        ],
        collection_account_id: COLLECTION_ACCOUNT_ID,
        collection_asset_id: SBTC_ASSET_ID,
        share_asset_id: RBTC_ASSET_ID,
        team_account_id: TEAM,
        profit_asset_id: TBD_ASSET_ID,
        profit_pool: PROFIT_POOL,
        reserved_mint_wallet: BOB,
        reserved_mint_asset_id: TBD_ASSET_ID,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
