//! # MIPS Module
//!
//! MESH Improvement Proposals (MIPs) are proposals (ballots) that can then be proposed and voted on
//! by all MESH token holders. If a ballot passes this community vote it is then passed to the
//! governance council to ratify (or reject).
//! - minimum of 5,000 MESH needs to be staked by the proposer of the ballot
//! in order to create a new ballot.
//! - minimum of 100,000 MESH (quorum) needs to vote in favour of the ballot in order for the
//! ballot to be considered by the governing committee.
//! - ballots run for 1 week
//! - a simple majority is needed to pass the ballot so that it heads for the
//! next stage (governing committee)
//!
//! ## Overview
//!
//! The Asset module provides functions for:
//!
//! - Creating Mesh Improvement Proposals
//! - Voting on Mesh Improvement Proposals
//! - Governance committee to ratify or reject proposals
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `propose` - Token holders can propose a new ballot.
//! - `vote` - Token holders can vote on a ballot.
//!
//! ### Public Functions
//!
//! - `end_block` - Returns details of the token

use codec::{Decode, Encode};
use rstd::prelude::*;
use sr_primitives::{
    traits::{Dispatchable, Hash, Zero},
    weights::SimpleDispatchInfo,
};
use srml_support::{
    decl_event, decl_module, decl_storage,
    dispatch::Result,
    ensure,
    traits::{Currency, Get, LockableCurrency, ReservableCurrency},
    Parameter,
};
use system::ensure_signed;

/// Mesh Improvement Proposal index. Used offchain.
pub type ProposalIndex = u32;

/// Balance
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// Represents a ballot
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct ProposalInfo<BlockNumber: Parameter, Proposal: Parameter> {
    /// When voting will end.
    end: BlockNumber,
    /// The proposal being voted on.
    proposal: Proposal,
}

impl<BlockNumber: Parameter, Proposal: Parameter> ProposalInfo<BlockNumber, Proposal> {
    /// Create a new instance.
    pub fn new(end: BlockNumber, proposal: Proposal, delay: BlockNumber) -> Self {
        ProposalInfo { end, proposal }
    }
}

/// For keeping track of proposal being voted on.
#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Votes<AccountId, Balance> {
    /// The proposal's unique index.
    index: ProposalIndex,
    /// The current set of voters that approved it.
    ayes: Vec<AccountId>,
    /// The current set of voters that rejected it.
    nays: Vec<AccountId>,
    /// Staked amount of approved votes.
    ayes_stake: Balance,
    /// Staked amount of rejected votes.
    nays_stake: Balance,
}

/// The module's configuration trait.
pub trait Trait: system::Trait {
    /// Currency type for this module.
    type Currency: ReservableCurrency<Self::AccountId>
        + LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

    /// A proposal is a dispatchable call
    type Proposal: Parameter + Dispatchable<Origin = Self::Origin>;

    /// The minimum amount to be used as a deposit for a proposal.
    type MinimumProposalDeposit: Get<BalanceOf<Self>>;

    /// Minimum stake a proposal must gather in order to be considered by the committee.
    type QuorumThreshold: Get<BalanceOf<Self>>;

    /// How long (in blocks) a ballot runs
    type VotingPeriod: Get<Self::BlockNumber>;

    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as MIPS {
        /// Proposals so far. Index can be used to keep track of MIPs off-chain.
        pub ProposalCount get(proposal_count): u32;

        // The hashes of the active proposals.
        //pub Proposals get(proposals): Vec<T::Hash>;

        /// Those who have locked a deposit.
        /// proposal index -> (deposit, proposer)
        pub DepositOf get(deposit_of): map T::Hash => Option<(T::AccountId, BalanceOf<T>)>;

        /// Actual proposal for a given hash, if it's current.
        /// proposal hash -> proposal
        pub Proposals get(proposals): linked_map T::Hash => Option<ProposalInfo<T::BlockNumber, T::Proposal>>;

        /// Votes on a given proposal, if it is ongoing.
        /// proposal hash -> voting info
        pub Voting get(voting): map T::Hash => Option<Votes<T::AccountId, BalanceOf<T>>>;

        /// Proposals that have met the quorum threshold to be put forward to a governance committee
        /// proposal hash -> proposal
        pub Referendums get(referendums): linked_map T::Hash => Option<ProposalInfo<T::BlockNumber, T::Proposal>>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        Balance = BalanceOf<T>,
        AccountId = <T as system::Trait>::AccountId,
    {
        Proposed(AccountId, Balance),
        Voted(AccountId),
    }
);

// The module's dispatchable functions.
decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        /// The minimum amount to be used as a deposit for a public referendum proposal.
        const MinimumProposalDeposit: BalanceOf<T> = T::MinimumProposalDeposit::get();

        /// Minimum stake a proposal must gather in order to be considered by the committee.
        const QuorumThreshold: BalanceOf<T> = T::QuorumThreshold::get();

        /// How long (in blocks) a ballot runs
        const VotingPeriod: T::BlockNumber = T::VotingPeriod::get();

        fn deposit_event() = default;

        /// A network member creates a Mesh Improvement Proposal by submitting a dispatchable which
        /// changes the network in someway. A minimum deposit is required to open a new proposal.
        ///
        /// # Arguments
        /// * `proposal` a dispatchable call
        /// * `deposit` minimum deposit value
        #[weight = SimpleDispatchInfo::FixedNormal(5_000_000)]
        pub fn propose(origin, proposal: Box<T::Proposal>, deposit: BalanceOf<T>) -> Result {
            let proposer = ensure_signed(origin)?;
            let proposal_hash = T::Hashing::hash_of(&proposal);

            // Pre conditions: caller must have min balance
            ensure!(deposit >= T::MinimumProposalDeposit::get(), "minimum deposit required to start a proposal");
            // Proposal must be new
            ensure!(!<Proposals<T>>::exists(proposal_hash), "duplicate proposals are not allowed");

            // Reserve the minimum deposit
            T::Currency::reserve(&proposer, deposit).map_err(|_| "proposer can't afford to lock minimum deposit")?;

            let index = Self::proposal_count();
            <ProposalCount>::mutate(|i| *i += 1);
            //<Proposals<T>>::mutate(|proposals| proposals.push(proposal_hash));

            <DepositOf<T>>::insert(proposal_hash, (proposer.clone(), deposit));

            let proposal_info = ProposalInfo {
                end: <system::Module<T>>::block_number() + T::VotingPeriod::get(),
                proposal: *proposal
            };
            <Proposals<T>>::insert(proposal_hash, proposal_info);

            let vote = Votes {
                index,
                ayes: vec![proposer.clone()],
                nays: vec![],
                ayes_stake: deposit,
                nays_stake: Zero::zero()
            };
            <Voting<T>>::insert(proposal_hash, vote);

            Self::deposit_event(RawEvent::Proposed(proposer, deposit));
            Ok(())
        }

        /// A network member can vote on any Mesh Improvement Proposal by selecting the hash that
        /// corresponds ot the dispatchable action and vote with some balance.
        ///
        /// # Arguments
        /// * `proposal` a dispatchable call
        /// * `deposit` minimum deposit value
        pub fn vote(origin, proposal_hash: T::Hash, #[compact] index: ProposalIndex) -> Result {
            let proposer = ensure_signed(origin)?;

            if let Some(proposal_info) = <Proposals<T>>::get(&proposal_hash) {
                Self::deposit_event(RawEvent::Voted(proposer));
            }

            Ok(())
        }

        /// When constructing a block check if it's time for a ballot to end. If ballot ends,
        /// proceed to ratification process.
        fn on_initialize(n: T::BlockNumber) {
            if let Err(e) = Self::end_block(n) {
            }
        }

    }
}

impl<T: Trait> Module<T> {
    /// Retrieve all proposals that need to be closed as of block `n`.
    pub fn proposals_maturing_at(
        n: T::BlockNumber,
    ) -> Vec<(T::Hash, ProposalInfo<T::BlockNumber, T::Proposal>)> {
        <Proposals<T>>::enumerate()
            .filter(|(proposal_hash, proposal_info)| proposal_info.end == n)
            .collect()
    }

    // Private functions

    /// Runs the following procedure:
    /// 1. Find all proposals that need to end as of this block and close voting
    /// 2. Tally votes
    /// 3. Submit any proposals that meet the quorum threshold, to the governance committee
    fn end_block(block_number: T::BlockNumber) -> Result {
        sr_primitives::print("end_block");

        // Tally up votes for matured proposals
        for (hash, info) in Self::proposals_maturing_at(block_number).into_iter() {
            Self::tally_votes(hash, info);
        }

        Ok(())
    }

    ///
    fn tally_votes(
        proposal_hash: T::Hash,
        proposal_info: ProposalInfo<T::BlockNumber, T::Proposal>,
    ) {
        if let Some(voting) = <Voting<T>>::get(proposal_hash) {
            let net_stake = voting.ayes_stake - voting.nays_stake;

            if net_stake >= T::QuorumThreshold::get() {
                <Referendums<T>>::insert(proposal_hash.clone(), proposal_info);
            }
        }
    }
}

// tests for this module
//#[cfg(test)]
//mod tests {
//    use super::*;
//
//    use crate::{balances, identity, staking};
//    use sr_io::{with_externalities, TestExternalities};
//    use sr_primitives::weights::Weight;
//    use sr_primitives::{
//        testing::{Header, UintAuthorityId},
//        traits::{BlakeTwo256, ConvertInto, IdentityLookup, OpaqueKeys, Verify},
//        AnySignature, Perbill,
//    };
//    use srml_support::traits::Currency;
//    use srml_support::{assert_ok, impl_outer_dispatch, impl_outer_origin, parameter_types};
//    use substrate_primitives::{Blake2Hasher, H256};
//
//    impl_outer_origin! {
//        pub enum Origin for Test {}
//    }
//
//    impl_outer_dispatch! {
//        pub enum Call for Test where origin: Origin {
//            balances::Balamces,
//        }
//    }
//
//    #[derive(Clone, Eq, PartialEq)]
//    pub struct Test;
//
//    parameter_types! {
//        pub const BlockHashCount: u64 = 250;
//        pub const MaximumBlockWeight: Weight = 1024;
//        pub const MaximumBlockLength: u32 = 2 * 1024;
//        pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
//    }
//
//    impl system::Trait for Test {
//        type Origin = Origin;
//        type Call = ();
//        type Index = u64;
//        type BlockNumber = u64;
//        type Hash = H256;
//        type Hashing = BlakeTwo256;
//        type AccountId = u64;
//        type Lookup = IdentityLookup<Self::AccountId>;
//        type Header = Header;
//        type WeightMultiplierUpdate = ();
//        type Event = ();
//        type BlockHashCount = BlockHashCount;
//        type MaximumBlockWeight = MaximumBlockWeight;
//        type MaximumBlockLength = MaximumBlockLength;
//        type AvailableBlockRatio = AvailableBlockRatio;
//        type Version = ();
//    }
//
//    impl identity::Trait for Test {
//        type Event = ();
//    }
//
//    parameter_types! {
//        pub const ExistentialDeposit: u64 = 0;
//        pub const TransferFee: u64 = 0;
//        pub const CreationFee: u64 = 0;
//        pub const TransactionBaseFee: u64 = 0;
//        pub const TransactionByteFee: u64 = 0;
//    }
//
//    impl balances::Trait for Test {
//        type Balance = u128;
//        type OnFreeBalanceZero = ();
//        type OnNewAccount = ();
//        type Event = ();
//        type TransactionPayment = ();
//        type DustRemoval = ();
//        type TransferPayment = ();
//        type ExistentialDeposit = ExistentialDeposit;
//        type TransferFee = TransferFee;
//        type CreationFee = CreationFee;
//        type TransactionBaseFee = TransactionBaseFee;
//        type TransactionByteFee = TransactionByteFee;
//        type WeightToFee = ConvertInto;
//        type Identity = identity::Module<Test>;
//    }
//
//    parameter_types! {
//        pub const MinimumPeriod: u64 = 3;
//    }
//
//    impl timestamp::Trait for Test {
//        type Moment = u64;
//        type OnTimestampSet = ();
//        type MinimumPeriod = MinimumPeriod;
//    }
//
//    impl Trait for Test {
//        type Currency = balances::Module<Test>;
//        type Proposal = Call;
//        type MinimumProposalDeposit = u64;
//        type VotingPeriod = u64;
//        type Event = ();
//    }
//
//    type MIPS = Module<Test>;
//
//    fn new_test_ext() -> TestExternalities<Blake2Hasher> {
//        system::GenesisConfig::default()
//            .build_storage::<Test>()
//            .unwrap()
//            .into()
//    }
//
//    #[test]
//    fn should_start_a_proposal() {
//        with_externalities(&mut new_test_ext(), || {
//            assert_ok!(MIPS::propose(Origin::signed(1), 42));
//            //            assert_eq!(MIPS::something(), Some(42));
//        });
//    }
//}
