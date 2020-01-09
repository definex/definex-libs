#![cfg(test)]

use super::*;
use primitives::{Blake2Hasher, H256};
use support::{impl_outer_origin, parameter_types};
// The testing primitives are very useful for avoiding having to work with signatures
// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.
use balances;

#[allow(unused_imports)]
use sr_primitives::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup, OnFinalize, OnInitialize},
    Perbill,
};

#[derive(Clone, Eq, PartialEq)]
pub struct TestRuntime;

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

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
    type WeightMultiplierUpdate = ();
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
}
parameter_types! {
    pub const ExistentialDeposit: u64 = 0;
    pub const TransferFee: u64 = 0;
    pub const CreationFee: u64 = 0;
    pub const TransactionBaseFee: u64 = 0;
    pub const TransactionByteFee: u64 = 0;
}
impl balances::Trait for TestRuntime {
    type Balance = u128;
    type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type Event = ();
    type TransactionPayment = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
    type TransactionBaseFee = TransactionBaseFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = ();
}
impl sudo::Trait for TestRuntime {
    type Event = ();

    // this is a wild guess ^_^
    type Proposal = Call<TestRuntime>;
}

impl generic_asset::Trait for TestRuntime {
    type Event = ();
    type Balance = u128;
    type AssetId = u32;
}
impl Trait for TestRuntime {
    type Event = ();
    type OnAssetMint = ();
    type OnAssetCreate = ();
    type OnAssetTransfer = ();
    type OnAssetBurn = ();
    type BeforeAssetMint = ();
    type BeforeAssetCreate = ();
    type BeforeAssetTransfer = ();
    type BeforeAssetBurn = ();
}

pub type Assets = Module<TestRuntime>;

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
    let mut t = system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();

    sudo::GenesisConfig::<TestRuntime> { key: 1 }
        .assimilate_storage(&mut t)
        .unwrap();

    // We use default for brevity, but you can configure as desired if needed.
    // balances::GenesisConfig::<Test>::default().assimilate_storage(&mut t).unwrap();
    // GenesisConfig::<TestRuntime> {
    //     phase_infos: vec![
    //         (PHASE1_QUOTA, PHASE1_EXCHANGE),
    //         (PHASE2_QUOTA, PHASE2_EXCHANGE),
    //         (PHASE3_QUOTA, PHASE3_EXCHANGE),
    //         (PHASE4_QUOTA, PHASE4_EXCHANGE),
    //         (PHASE5_QUOTA, PHASE5_EXCHANGE),
    //     ],
    //     collection_account_id: COLLECTION_ACCOUNT_ID,
    // }
    // .assimilate_storage(&mut t)
    // .unwrap();

    t.into()
}
