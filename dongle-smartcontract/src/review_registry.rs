use crate::constants::{RATING_MAX, RATING_MIN};
use crate::errors::ContractError;
use crate::events::publish_review_event;
use crate::storage_keys::StorageKey;
use crate::types::{Review, ReviewAction};
use soroban_sdk::{contract, contractimpl, Address, Env, String};

#[contract]
pub struct ReviewRegistry;

#[contractimpl]
impl ReviewRegistry {
    pub fn add_review(
        env: Env,
        project_id: u64,
        reviewer: Address,
        rating: u32,
        ipfs_cid: Option<String>,
    ) -> Result<(), ContractError> {
        reviewer.require_auth();

        if !(RATING_MIN..=RATING_MAX).contains(&rating) {
            return Err(ContractError::InvalidRating);
        }

        let review_key = StorageKey::Review(project_id, reviewer.clone());
        if env.storage().persistent().has(&review_key) {
            return Err(ContractError::DuplicateReview);
        }

        let review = Review {
            project_id,
            reviewer: reviewer.clone(),
            rating,
            ipfs_cid: ipfs_cid.clone(),
        };

        env.storage().persistent().set(&review_key, &review);

        publish_review_event(
            &env,
            project_id,
            reviewer,
            ReviewAction::Submitted,
            ipfs_cid,
        );

        Ok(())
    }

    pub fn update_review(
        env: Env,
        project_id: u64,
        reviewer: Address,
        rating: u32,
        ipfs_cid: Option<String>,
    ) -> Result<(), ContractError> {
        reviewer.require_auth();

        if !(RATING_MIN..=RATING_MAX).contains(&rating) {
            return Err(ContractError::InvalidRating);
        }

        let review_key = StorageKey::Review(project_id, reviewer.clone());
        let mut existing: Review = env
            .storage()
            .persistent()
            .get(&review_key)
            .ok_or(ContractError::ReviewNotFound)?;

        if existing.reviewer != reviewer {
            return Err(ContractError::NotReviewOwner);
        }

        existing.rating = rating;
        existing.ipfs_cid = ipfs_cid.clone();
        env.storage().persistent().set(&review_key, &existing);

        publish_review_event(&env, project_id, reviewer, ReviewAction::Updated, ipfs_cid);

        Ok(())
    }

    pub fn delete_review(
        env: Env,
        project_id: u64,
        reviewer: Address,
    ) -> Result<(), ContractError> {
        reviewer.require_auth();

        let review_key = StorageKey::Review(project_id, reviewer.clone());
        let existing: Review = env
            .storage()
            .persistent()
            .get(&review_key)
            .ok_or(ContractError::ReviewNotFound)?;

        if existing.reviewer != reviewer {
            return Err(ContractError::NotReviewOwner);
        }

        env.storage().persistent().remove(&review_key);

        publish_review_event(&env, project_id, reviewer, ReviewAction::Deleted, None);

        Ok(())
    }

    pub fn get_review(env: Env, project_id: u64, reviewer: Address) -> Option<Review> {
        env.storage()
            .persistent()
            .get(&StorageKey::Review(project_id, reviewer))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::ReviewEventData;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Env, IntoVal, String,
    };

    #[test]
    fn test_invalid_rating_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let reviewer = Address::generate(&env);
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        let zero = client.try_add_review(&1, &reviewer, &0, &None);
        let six = client.try_add_review(&1, &reviewer, &6, &None);

        assert_eq!(zero, Err(Ok(ContractError::InvalidRating)));
        assert_eq!(six, Err(Ok(ContractError::InvalidRating)));
    }

    #[test]
    fn test_duplicate_review_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let reviewer = Address::generate(&env);
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        client.add_review(&7, &reviewer, &5, &None);
        let duplicate = client.try_add_review(&7, &reviewer, &4, &None);
        assert_eq!(duplicate, Err(Ok(ContractError::DuplicateReview)));
    }

    #[test]
    fn test_only_owner_can_update_or_delete() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        client.add_review(&11, &owner, &5, &None);

        let update_other = client.try_update_review(&11, &other, &3, &None);
        let delete_other = client.try_delete_review(&11, &other);

        assert_eq!(update_other, Err(Ok(ContractError::ReviewNotFound)));
        assert_eq!(delete_other, Err(Ok(ContractError::ReviewNotFound)));
    }

    #[test]
    fn test_add_review_event() {
        let env = Env::default();
        env.mock_all_auths();
        let reviewer = Address::generate(&env);
        let ipfs_cid = String::from_str(&env, "QmHash");
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        client.add_review(&1, &reviewer, &5, &Some(ipfs_cid.clone()));

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let (_, topics, data) = events.last().unwrap();

        assert_eq!(topics.len(), 4);

        let topic0: soroban_sdk::Symbol = topics.get(0).unwrap().into_val(&env);
        let topic1: soroban_sdk::Symbol = topics.get(1).unwrap().into_val(&env);
        let topic2: u64 = topics.get(2).unwrap().into_val(&env);
        let topic3: Address = topics.get(3).unwrap().into_val(&env);

        assert_eq!(topic0, soroban_sdk::symbol_short!("REVIEW"));
        assert_eq!(topic1, soroban_sdk::symbol_short!("SUBMITTED"));
        assert_eq!(topic2, 1u64);
        assert_eq!(topic3, reviewer);

        let event_data: ReviewEventData = data.into_val(&env);
        assert_eq!(event_data.project_id, 1);
        assert_eq!(event_data.reviewer, reviewer);
        assert_eq!(event_data.action, ReviewAction::Submitted);
        assert_eq!(event_data.timestamp, env.ledger().timestamp());
        assert_eq!(event_data.ipfs_cid, Some(ipfs_cid));
    }

    #[test]
    fn test_update_review_event() {
        let env = Env::default();
        env.mock_all_auths();
        let reviewer = Address::generate(&env);
        let ipfs_cid = String::from_str(&env, "QmHash2");
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        client.add_review(&1, &reviewer, &5, &None);
        client.update_review(&1, &reviewer, &4, &Some(ipfs_cid.clone()));

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let (_, topics, data) = events.last().unwrap();
        let topic1: soroban_sdk::Symbol = topics.get(1).unwrap().into_val(&env);
        assert_eq!(topic1, soroban_sdk::symbol_short!("UPDATED"));

        let event_data: ReviewEventData = data.into_val(&env);
        assert_eq!(event_data.project_id, 1);
        assert_eq!(event_data.reviewer, reviewer);
        assert_eq!(event_data.action, ReviewAction::Updated);
        assert_eq!(event_data.timestamp, env.ledger().timestamp());
        assert_eq!(event_data.ipfs_cid, Some(ipfs_cid));
    }

    #[test]
    fn test_delete_review_event() {
        let env = Env::default();
        env.mock_all_auths();
        let reviewer = Address::generate(&env);
        let contract_id = env.register_contract(None, ReviewRegistry);
        let client = ReviewRegistryClient::new(&env, &contract_id);

        client.add_review(&1, &reviewer, &5, &None);
        client.delete_review(&1, &reviewer);

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let (_, topics, data) = events.last().unwrap();
        let topic1: soroban_sdk::Symbol = topics.get(1).unwrap().into_val(&env);
        assert_eq!(topic1, soroban_sdk::symbol_short!("DELETED"));

        let event_data: ReviewEventData = data.into_val(&env);
        assert_eq!(event_data.project_id, 1);
        assert_eq!(event_data.reviewer, reviewer);
        assert_eq!(event_data.action, ReviewAction::Deleted);
        assert_eq!(event_data.timestamp, env.ledger().timestamp());
        assert_eq!(event_data.ipfs_cid, None);
    }
}
