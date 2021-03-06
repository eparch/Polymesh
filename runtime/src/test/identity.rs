use crate::{
    balances,
    identity::{self, ClaimValue, DataTypes, SigningItemWithAuth, TargetIdAuthorization},
    test::storage::{build_ext, register_keyring_account, TestStorage},
};
use primitives::{Key, Permission, Signer, SignerType, SigningItem};

use codec::Encode;
use sr_io::with_externalities;
use srml_support::{assert_err, assert_ok, traits::Currency};
use substrate_primitives::H512;
use test_client::AccountKeyring;

type Identity = identity::Module<TestStorage>;
type Balances = balances::Module<TestStorage>;
type System = system::Module<TestStorage>;
type Timestamp = timestamp::Module<TestStorage>;

type Origin = <TestStorage as system::Trait>::Origin;

#[test]
fn only_claim_issuers_can_add_claims() {
    with_externalities(&mut build_ext(), || {
        let owner_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let issuer_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let issuer = AccountKeyring::Bob.public();
        let claim_issuer_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
        let claim_issuer = AccountKeyring::Charlie.public();

        let claim_value = ClaimValue {
            data_type: DataTypes::VecU8,
            value: "some_value".as_bytes().to_vec(),
        };

        assert_ok!(Identity::add_claim(
            Origin::signed(claim_issuer.clone()),
            claim_issuer_did.clone(),
            "some_key".as_bytes().to_vec(),
            claim_issuer_did.clone(),
            100u64,
            claim_value.clone()
        ));

        assert_err!(
            Identity::add_claim(
                Origin::signed(claim_issuer.clone()),
                owner_did,
                "some_key".as_bytes().to_vec(),
                issuer_did.clone(),
                100u64,
                claim_value.clone()
            ),
            "did_issuer must be a claim issuer or master key for DID"
        );
        assert_err!(
            Identity::add_claim(
                Origin::signed(issuer),
                issuer_did,
                "some_key".as_bytes().to_vec(),
                claim_issuer_did,
                100u64,
                claim_value
            ),
            "Sender must hold a claim issuer\'s signing key"
        );
    });
}

/// TODO Add `Signer::Identity(..)` test.
#[test]
fn only_master_or_signing_keys_can_authenticate_as_an_identity() {
    with_externalities(&mut build_ext(), || {
        let owner_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let owner_signer = Signer::Key(Key::from(AccountKeyring::Alice.public().0));

        let a_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let a = Origin::signed(AccountKeyring::Bob.public());
        let b_did = register_keyring_account(AccountKeyring::Dave).unwrap();

        let charlie_key = Key::from(AccountKeyring::Charlie.public().0);
        let charlie_signer = Signer::Key(charlie_key);
        let charlie_signing_item =
            SigningItem::new(charlie_signer.clone(), vec![Permission::Admin]);

        assert_ok!(Identity::add_signing_items(
            a.clone(),
            a_did,
            vec![charlie_signing_item]
        ));
        assert_ok!(Identity::authorize_join_to_identity(
            Origin::signed(AccountKeyring::Charlie.public()),
            a_did
        ));

        // Check master key on master and signing_keys.
        assert!(Identity::is_signer_authorized(owner_did, &owner_signer));
        assert!(Identity::is_signer_authorized(a_did, &charlie_signer));

        assert!(Identity::is_signer_authorized(b_did, &charlie_signer) == false);

        // ... and remove that key.
        assert_ok!(Identity::remove_signing_items(
            a.clone(),
            a_did.clone(),
            vec![charlie_signer.clone()]
        ));
        assert!(Identity::is_signer_authorized(a_did, &charlie_signer) == false);
    });
}

#[test]
fn revoking_claims() {
    with_externalities(&mut build_ext(), || {
        let owner_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let owner = Origin::signed(AccountKeyring::Alice.public());
        let issuer_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let issuer = Origin::signed(AccountKeyring::Bob.public());
        let claim_issuer_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
        let claim_issuer = Origin::signed(AccountKeyring::Charlie.public());

        assert_ok!(Identity::add_claim_issuer(
            owner.clone(),
            owner_did,
            claim_issuer_did
        ));

        let claim_value = ClaimValue {
            data_type: DataTypes::VecU8,
            value: "some_value".as_bytes().to_vec(),
        };

        assert_ok!(Identity::add_claim(
            claim_issuer.clone(),
            claim_issuer_did,
            "some_key".as_bytes().to_vec(),
            claim_issuer_did,
            100u64,
            claim_value.clone()
        ));

        assert_err!(
            Identity::revoke_claim(
                issuer.clone(),
                issuer_did,
                "some_key".as_bytes().to_vec(),
                claim_issuer_did
            ),
            "Sender must hold a claim issuer\'s signing key"
        );

        assert_ok!(Identity::revoke_claim(
            claim_issuer.clone(),
            owner_did,
            "some_key".as_bytes().to_vec(),
            claim_issuer_did
        ));
    });
}

#[test]
fn only_master_key_can_add_signing_key_permissions() {
    with_externalities(
        &mut build_ext(),
        &only_master_key_can_add_signing_key_permissions_with_externalities,
    );
}

fn only_master_key_can_add_signing_key_permissions_with_externalities() {
    let bob_key = Key::from(AccountKeyring::Bob.public().0);
    let charlie_key = Key::from(AccountKeyring::Charlie.public().0);
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob = Origin::signed(AccountKeyring::Bob.public());
    let charlie = Origin::signed(AccountKeyring::Charlie.public());

    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_did,
        vec![SigningItem::from(bob_key), SigningItem::from(charlie_key)]
    ));
    assert_ok!(Identity::authorize_join_to_identity(bob.clone(), alice_did));
    assert_ok!(Identity::authorize_join_to_identity(charlie, alice_did));

    // Only `alice` is able to update `bob`'s permissions and `charlie`'s permissions.
    assert_ok!(Identity::set_permission_to_signer(
        alice.clone(),
        alice_did,
        Signer::Key(bob_key),
        vec![Permission::Operator]
    ));
    assert_ok!(Identity::set_permission_to_signer(
        alice.clone(),
        alice_did,
        Signer::Key(charlie_key),
        vec![Permission::Admin, Permission::Operator]
    ));

    // Bob tries to get better permission by himself at `alice` Identity.
    assert_err!(
        Identity::set_permission_to_signer(
            bob.clone(),
            alice_did,
            Signer::Key(bob_key),
            vec![Permission::Full]
        ),
        "Only master key of an identity is able to execute this operation"
    );

    // Bob tries to remove Charlie's permissions at `alice` Identity.
    assert_err!(
        Identity::set_permission_to_signer(bob, alice_did, Signer::Key(charlie_key), vec![]),
        "Only master key of an identity is able to execute this operation"
    );

    // Alice over-write some permissions.
    assert_ok!(Identity::set_permission_to_signer(
        alice,
        alice_did,
        Signer::Key(bob_key),
        vec![]
    ));
}

#[test]
fn add_signing_keys_with_specific_type() {
    with_externalities(
        &mut build_ext(),
        &add_signing_keys_with_specific_type_with_externalities,
    );
}

/// It tests that signing key can be added using non-default key type
/// (`SignerType::External`).
fn add_signing_keys_with_specific_type_with_externalities() {
    let charlie_key = Key::from(AccountKeyring::Charlie.public().0);
    let dave_key = Key::from(AccountKeyring::Dave.public().0);

    // Create keys using non-default type.
    let charlie_signing_key = SigningItem {
        signer: Signer::Key(charlie_key),
        signer_type: SignerType::Relayer,
        permissions: vec![],
    };
    let dave_signing_key = SigningItem {
        signer: Signer::Key(dave_key),
        signer_type: SignerType::Multisig,
        permissions: vec![],
    };

    // Add signing keys with non-default type.
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    assert_ok!(Identity::add_signing_items(
        alice,
        alice_did,
        vec![charlie_signing_key, dave_signing_key.clone()]
    ));

    // Register did with non-default type.
    let bob = AccountKeyring::Bob.public();
    Balances::make_free_balance_be(&bob, 5_000);
    assert_ok!(Identity::register_did(
        Origin::signed(bob),
        vec![dave_signing_key]
    ));
}

/// It verifies that frozen keys are recovered after `unfreeze` call.
#[test]
fn freeze_signing_keys_test() {
    with_externalities(&mut build_ext(), &freeze_signing_keys_with_externalities);
}

fn freeze_signing_keys_with_externalities() {
    let (bob_key, charlie_key, dave_key) = (
        Key::from(AccountKeyring::Bob.public().0),
        Key::from(AccountKeyring::Charlie.public().0),
        Key::from(AccountKeyring::Dave.public().0),
    );
    let bob = Origin::signed(AccountKeyring::Bob.public());
    let charlie = Origin::signed(AccountKeyring::Charlie.public());
    let dave = Origin::signed(AccountKeyring::Dave.public());

    let bob_signing_key = SigningItem::new(Signer::Key(bob_key), vec![Permission::Admin]);
    let charlie_signing_key =
        SigningItem::new(Signer::Key(charlie_key), vec![Permission::Operator]);
    let dave_signing_key = SigningItem::from(dave_key);

    // Add signing keys.
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());

    let signing_keys_v1 = vec![bob_signing_key.clone(), charlie_signing_key];
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_did,
        signing_keys_v1.clone()
    ));
    assert_ok!(Identity::authorize_join_to_identity(bob.clone(), alice_did));
    assert_ok!(Identity::authorize_join_to_identity(
        charlie.clone(),
        alice_did
    ));

    assert_eq!(
        Identity::is_signer_authorized(alice_did, &Signer::Key(bob_key)),
        true
    );

    // Freeze signing keys: bob & charlie.
    assert_err!(
        Identity::freeze_signing_keys(bob.clone(), alice_did),
        "Only master key of an identity is able to execute this operation"
    );
    assert_ok!(Identity::freeze_signing_keys(alice.clone(), alice_did));

    assert_eq!(
        Identity::is_signer_authorized(alice_did, &Signer::Key(bob_key)),
        false
    );

    // Add new signing keys.
    let signing_keys_v2 = vec![dave_signing_key.clone()];
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_did,
        signing_keys_v2.clone()
    ));
    assert_ok!(Identity::authorize_join_to_identity(dave, alice_did));
    assert_eq!(
        Identity::is_signer_authorized(alice_did, &Signer::Key(dave_key)),
        false
    );

    // update permission of frozen keys.
    assert_ok!(Identity::set_permission_to_signer(
        alice.clone(),
        alice_did,
        Signer::Key(bob_key),
        vec![Permission::Operator]
    ));

    // unfreeze all
    assert_err!(
        Identity::unfreeze_signing_keys(bob.clone(), alice_did),
        "Only master key of an identity is able to execute this operation"
    );
    assert_ok!(Identity::unfreeze_signing_keys(alice.clone(), alice_did));

    assert_eq!(
        Identity::is_signer_authorized(alice_did, &Signer::Key(dave_key)),
        true
    );
}

/// It double-checks that frozen keys are removed too.
#[test]
fn remove_frozen_signing_keys_test() {
    with_externalities(
        &mut build_ext(),
        &remove_frozen_signing_keys_with_externalities,
    );
}

fn remove_frozen_signing_keys_with_externalities() {
    let (bob_key, charlie_key) = (
        Key::from(AccountKeyring::Bob.public().0),
        Key::from(AccountKeyring::Charlie.public().0),
    );

    let bob_signing_key = SigningItem::new(Signer::Key(bob_key), vec![Permission::Admin]);
    let charlie_signing_key =
        SigningItem::new(Signer::Key(charlie_key), vec![Permission::Operator]);

    // Add signing keys.
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());

    let signing_keys_v1 = vec![bob_signing_key, charlie_signing_key.clone()];
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_did,
        signing_keys_v1.clone()
    ));
    assert_ok!(Identity::authorize_join_to_identity(
        Origin::signed(AccountKeyring::Bob.public()),
        alice_did
    ));
    assert_ok!(Identity::authorize_join_to_identity(
        Origin::signed(AccountKeyring::Charlie.public()),
        alice_did
    ));

    // Freeze all signing keys
    assert_ok!(Identity::freeze_signing_keys(alice.clone(), alice_did));

    // Remove Bob's key.
    assert_ok!(Identity::remove_signing_items(
        alice.clone(),
        alice_did,
        vec![Signer::Key(bob_key)]
    ));
    // Check DidRecord.
    let did_rec = Identity::did_records(alice_did);
    assert_eq!(did_rec.signing_items, vec![charlie_signing_key]);
}

#[test]
fn add_claim_issuer_tests() {
    with_externalities(&mut build_ext(), &add_claim_issuer_tests_with_externalities);
}

fn add_claim_issuer_tests_with_externalities() {
    // Register identities
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let charlie = Origin::signed(AccountKeyring::Charlie.public());

    // Check `add_claim_issuer` constraints.
    assert_ok!(Identity::add_claim_issuer(
        alice.clone(),
        alice_did,
        bob_did
    ));
    assert_err!(
        Identity::add_claim_issuer(charlie, alice_did, bob_did),
        "Only master key of an identity is able to execute this operation"
    );
    assert_err!(
        Identity::add_claim_issuer(alice, alice_did, alice_did),
        "Master key cannot add itself as claim issuer"
    );
}

#[test]
fn enforce_uniqueness_keys_in_identity_tests() {
    with_externalities(&mut build_ext(), &enforce_uniqueness_keys_in_identity);
}

fn enforce_uniqueness_keys_in_identity() {
    let unique_error = "One signing key can only belong to one DID";
    // Register identities
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    let bob = Origin::signed(AccountKeyring::Bob.public());

    // Check external signed key uniqueness.
    let charlie_key = Key::from(AccountKeyring::Charlie.public().0);
    let charlie_sk = SigningItem::new(Signer::Key(charlie_key), vec![Permission::Operator]);
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_id,
        vec![charlie_sk.clone()]
    ));
    assert_ok!(Identity::authorize_join_to_identity(
        Origin::signed(AccountKeyring::Charlie.public()),
        alice_id
    ));

    assert_err!(
        Identity::add_signing_items(bob.clone(), bob_id, vec![charlie_sk]),
        unique_error
    );

    // Check non-external signed key non-uniqueness.
    let dave_key = Key::from(AccountKeyring::Dave.public().0);
    let dave_sk = SigningItem {
        signer: Signer::Key(dave_key),
        signer_type: SignerType::Multisig,
        permissions: vec![Permission::Operator],
    };
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_id,
        vec![dave_sk.clone()]
    ));
    assert_ok!(Identity::add_signing_items(
        bob.clone(),
        bob_id,
        vec![dave_sk]
    ));

    // Check that master key acts like external signed key.
    let bob_key = Key::from(AccountKeyring::Bob.public().0);
    let bob_sk_as_mutisig = SigningItem {
        signer: Signer::Key(bob_key),
        signer_type: SignerType::Multisig,
        permissions: vec![Permission::Operator],
    };
    assert_err!(
        Identity::add_signing_items(alice.clone(), alice_id, vec![bob_sk_as_mutisig]),
        unique_error
    );

    let bob_sk = SigningItem::new(Signer::Key(bob_key), vec![Permission::Admin]);
    assert_err!(
        Identity::add_signing_items(alice.clone(), alice_id, vec![bob_sk]),
        unique_error
    );
}

#[test]
fn add_remove_signing_identities() {
    with_externalities(
        &mut build_ext(),
        &add_remove_signing_identities_with_externalities,
    );
}

fn add_remove_signing_identities_with_externalities() {
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    let bob = Origin::signed(AccountKeyring::Bob.public());

    let charlie_id = register_keyring_account(AccountKeyring::Charlie).unwrap();
    let charlie = Origin::signed(AccountKeyring::Charlie.public());
    let dave_id = register_keyring_account(AccountKeyring::Dave).unwrap();

    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_id,
        vec![SigningItem::from(bob_id), SigningItem::from(charlie_id)]
    ));
    assert_ok!(Identity::authorize_join_to_identity(bob, alice_id));
    assert_ok!(Identity::authorize_join_to_identity(charlie, alice_id));
    assert_eq!(
        Identity::is_signer_authorized(alice_id, &Signer::Identity(bob_id)),
        true
    );

    assert_ok!(Identity::remove_signing_items(
        alice.clone(),
        alice_id,
        vec![Signer::Identity(bob_id), Signer::Identity(dave_id)]
    ));

    let alice_rec = Identity::did_records(alice_id);
    assert_eq!(alice_rec.signing_items, vec![SigningItem::from(charlie_id)]);

    // Check is_authorized_identity
    assert_eq!(
        Identity::is_signer_authorized(alice_id, &Signer::Identity(charlie_id)),
        true
    );
    assert_eq!(
        Identity::is_signer_authorized(alice_id, &Signer::Identity(bob_id)),
        false
    );
}

#[test]
fn two_step_join_id() {
    with_externalities(&mut build_ext(), &two_step_join_id_with_ext);
}

fn two_step_join_id_with_ext() {
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    let bob = Origin::signed(AccountKeyring::Bob.public());

    let c_sk = SigningItem::new(
        Signer::Key(Key::from(AccountKeyring::Charlie.public().0)),
        vec![Permission::Operator],
    );
    let d_sk = SigningItem::new(
        Signer::Key(Key::from(AccountKeyring::Dave.public().0)),
        vec![Permission::Full],
    );
    let e_sk = SigningItem::new(
        Signer::Key(Key::from(AccountKeyring::Eve.public().0)),
        vec![Permission::Full],
    );

    // Check 1-to-1 relation between key and identity.
    let signing_keys = vec![c_sk.clone(), d_sk.clone(), e_sk.clone()];
    assert_ok!(Identity::add_signing_items(
        alice.clone(),
        alice_id,
        signing_keys.clone()
    ));
    assert_ok!(Identity::add_signing_items(
        bob.clone(),
        bob_id,
        signing_keys
    ));
    assert_eq!(
        Identity::is_signer_authorized(alice_id, &c_sk.signer),
        false
    );

    let charlie = Origin::signed(AccountKeyring::Charlie.public());
    assert_ok!(Identity::authorize_join_to_identity(
        charlie.clone(),
        alice_id
    ));
    assert_eq!(Identity::is_signer_authorized(alice_id, &c_sk.signer), true);

    assert_err!(
        Identity::authorize_join_to_identity(charlie, bob_id),
        "Key is already linked to an identity"
    );
    assert_eq!(Identity::is_signer_authorized(bob_id, &c_sk.signer), false);

    // Check after remove a signing key.
    let dave = Origin::signed(AccountKeyring::Dave.public());
    assert_ok!(Identity::authorize_join_to_identity(dave, alice_id));
    assert_eq!(Identity::is_signer_authorized(alice_id, &d_sk.signer), true);
    assert_ok!(Identity::remove_signing_items(
        alice.clone(),
        alice_id,
        vec![d_sk.signer.clone()]
    ));
    assert_eq!(
        Identity::is_signer_authorized(alice_id, &d_sk.signer),
        false
    );

    // Check remove pre-authorization from master and itself.
    assert_err!(
        Identity::unauthorized_join_to_identity(alice.clone(), e_sk.signer.clone(), bob_id),
        "Account cannot remove this authorization"
    );
    assert_ok!(Identity::unauthorized_join_to_identity(
        alice,
        e_sk.signer.clone(),
        alice_id
    ));

    let eve = Origin::signed(AccountKeyring::Eve.public());
    assert_ok!(Identity::unauthorized_join_to_identity(
        eve,
        e_sk.signer,
        bob_id
    ));
}

#[test]
fn one_step_join_id() {
    with_externalities(&mut build_ext(), &one_step_join_id_with_ext);
}

fn one_step_join_id_with_ext() {
    let a_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let a_pub = AccountKeyring::Alice.public();
    let a = Origin::signed(a_pub.clone());
    let b_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    let c_id = register_keyring_account(AccountKeyring::Charlie).unwrap();
    let d_id = register_keyring_account(AccountKeyring::Dave).unwrap();

    let expires_at = 100u64;
    let authorization = TargetIdAuthorization {
        target_id: a_id.clone(),
        nonce: Identity::offchain_authorization_nonce(a_id),
        expires_at,
    };
    let auth_encoded = authorization.encode();

    let signatures = [
        AccountKeyring::Bob,
        AccountKeyring::Charlie,
        AccountKeyring::Dave,
    ]
    .into_iter()
    .map(|acc| H512::from(acc.sign(&auth_encoded)))
    .collect::<Vec<_>>();

    let signing_items_with_auth = vec![
        SigningItemWithAuth {
            signing_item: SigningItem::from(b_id.clone()),
            auth_signature: signatures[0].clone(),
        },
        SigningItemWithAuth {
            signing_item: SigningItem::from(c_id.clone()),
            auth_signature: signatures[1].clone(),
        },
        SigningItemWithAuth {
            signing_item: SigningItem::from(d_id.clone()),
            auth_signature: signatures[2].clone(),
        },
    ];

    assert_ok!(Identity::add_signing_items_with_authorization(
        a.clone(),
        a_id,
        expires_at,
        signing_items_with_auth[..2].to_owned()
    ));

    let signing_items = Identity::did_records(a_id).signing_items;
    assert_eq!(signing_items.iter().find(|si| **si == b_id).is_some(), true);
    assert_eq!(signing_items.iter().find(|si| **si == c_id).is_some(), true);

    // Check reply atack. Alice's nonce is different now.
    // NOTE: We need to force the increment of account's nonce manually.
    System::inc_account_nonce(&a_pub);

    assert_err!(
        Identity::add_signing_items_with_authorization(
            a.clone(),
            a_id,
            expires_at,
            signing_items_with_auth[2..].to_owned()
        ),
        "Invalid Authorization signature"
    );

    // Check revoke off-chain authorization.
    let e = Origin::signed(AccountKeyring::Eve.public());
    let e_id = register_keyring_account(AccountKeyring::Eve).unwrap();
    let eve_auth = TargetIdAuthorization {
        target_id: a_id.clone(),
        nonce: Identity::offchain_authorization_nonce(a_id),
        expires_at,
    };
    assert_ne!(authorization.nonce, eve_auth.nonce);

    let eve_signing_item_with_auth = SigningItemWithAuth {
        signing_item: SigningItem::from(e_id),
        auth_signature: H512::from(AccountKeyring::Eve.sign(eve_auth.encode().as_slice())),
    };

    assert_ok!(Identity::revoke_offchain_authorization(
        e,
        Signer::Identity(e_id),
        eve_auth
    ));
    assert_err!(
        Identity::add_signing_items_with_authorization(
            a,
            a_id.clone(),
            expires_at,
            vec![eve_signing_item_with_auth]
        ),
        "Authorization has been explicitly revoked"
    );

    // Check expire
    System::inc_account_nonce(&a_pub);
    Timestamp::set_timestamp(expires_at);

    let f = Origin::signed(AccountKeyring::Ferdie.public());
    let f_id = register_keyring_account(AccountKeyring::Ferdie).unwrap();
    let ferdie_auth = TargetIdAuthorization {
        target_id: a_id.clone(),
        nonce: Identity::offchain_authorization_nonce(a_id),
        expires_at,
    };
    let ferdie_signing_item_with_auth = SigningItemWithAuth {
        signing_item: SigningItem::from(f_id.clone()),
        auth_signature: H512::from(AccountKeyring::Eve.sign(ferdie_auth.encode().as_slice())),
    };

    assert_err!(
        Identity::add_signing_items_with_authorization(
            f,
            f_id,
            expires_at,
            vec![ferdie_signing_item_with_auth]
        ),
        "Offchain authorization has expired"
    );
}
