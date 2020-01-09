#![cfg(test)]

use super::*;

// use rstd::fmt::Debug;
// use primitives::{Blake2Hasher, H256};
use runtime_io::with_externalities;
use support::{assert_noop, assert_ok};
// The testing primitives are very useful for avoiding having to work with signatures
// or public keys. `u64` is used as the `AccountId` and no `Signature`s are required.

#[allow(unused_imports)]
use sr_primitives::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup, OnFinalize, OnInitialize},
    Perbill,
};

use mock::*;

#[test]
fn unittest_works() {
    dbg!("hello world");
}
