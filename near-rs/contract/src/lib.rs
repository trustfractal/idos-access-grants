extern crate near_sdk;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::serde::Serialize;
use near_sdk::serde_json::json;
use near_sdk::{env, near_bindgen, require, AccountId, EpochHeight, PublicKey};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct FractalRegistry {
    pub grants_by_id: LookupMap<String, Grant>,

    pub grant_ids_by_owner: LookupMap<AccountId, Vec<String>>,
    pub grant_ids_by_grantee: LookupMap<PublicKey, Vec<String>>,
    pub grant_ids_by_data_id: LookupMap<String, Vec<String>>,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Grant {
    owner: AccountId,
    grantee: PublicKey,
    data_id: String,
    locked_until: EpochHeight,
}

#[cfg(test)]
#[test]
fn derive_grant_id_example() {
    // Just to make sure we don't accidentally change the way we derive grant_ids.

    let grant = Grant{
        owner: "my-cool-account.near".parse().unwrap(),
        grantee: "secp256k1:qMoRgcoXai4mBPsdbHi1wfyxF9TdbPCF4qSDQTRP3TfescSRoUdSx6nmeQoN3aiwGzwMyGXAb1gUjBTv5AY8DXj".parse().unwrap(),
        data_id: "some data".into(),
        locked_until: 1337,
    };

    assert_eq!(
        "848a69fe2d9b5d82d92a56936aa00f499f7274e8233eedba07b676de9d4c91be",
        derive_grant_id(&grant)
    );
}

pub fn derive_grant_id(grant: &Grant) -> String {
    let id = format!(
        "{}{}{}{}",
        grant.owner,
        Into::<String>::into(&grant.grantee),
        grant.data_id,
        grant.locked_until,
    );

    hex::encode(env::keccak256(id.as_bytes()))
}

impl Default for FractalRegistry {
    fn default() -> Self {
        let grants_by_id = LookupMap::new(b"g");
        let grant_ids_by_owner = LookupMap::new(b"h");
        let grant_ids_by_grantee = LookupMap::new(b"i");
        let grant_ids_by_data_id = LookupMap::new(b"j");

        Self {
            grants_by_id,
            grant_ids_by_owner,
            grant_ids_by_grantee,
            grant_ids_by_data_id,
        }
    }
}

fn get_push_insert<K: BorshSerialize, V: BorshDeserialize + BorshSerialize + Clone>(
    collection: &mut LookupMap<K, Vec<V>>,
    key: &K,
    value: &V,
) {
    let mut value_vec = collection.get(key).unwrap_or_default();
    value_vec.push(value.clone());
    collection.insert(key, &value_vec);
}

fn remove_values<
    K: BorshSerialize,
    V: BorshDeserialize + BorshSerialize + Clone + std::cmp::PartialEq<V>,
>(
    collection: &mut LookupMap<K, Vec<V>>,
    key: &K,
    value: &V,
) {
    let mut value_vec = collection.get(key).unwrap();
    value_vec.retain(|id| *id != *value);
    collection.insert(key, &value_vec);
}

#[near_bindgen]
impl FractalRegistry {
    pub fn insert_grant(
        &mut self,
        grantee: PublicKey,
        data_id: String,
        locked_until: Option<EpochHeight>,
    ) {
        let owner = env::predecessor_account_id();

        let grant = Grant {
            owner: owner.clone(),
            grantee: grantee.clone(),
            data_id: data_id.clone(),
            locked_until: locked_until.unwrap_or(0),
        };

        let grant_id = derive_grant_id(&grant);

        require!(
            !self.grants_by_id.contains_key(&grant_id),
            "Grant already exists"
        );

        self.grants_by_id.insert(&grant_id, &grant);

        get_push_insert(&mut self.grant_ids_by_owner, &owner, &grant_id);
        get_push_insert(&mut self.grant_ids_by_grantee, &grantee, &grant_id);
        get_push_insert(&mut self.grant_ids_by_data_id, &data_id, &grant_id);

        env::log_str(&format!(
            "EVENT_JSON:{}",
            json!({
                "standard": "FractalRegistry",
                "version": "0",
                "event": "grant_inserted",
                "data": {
                    "owner": owner,
                    "grantee": grantee,
                    "data_id": data_id,
                    "locked_until": locked_until.unwrap_or(0),
                },
            })
        ))
    }

    pub fn delete_grant(
        &mut self,
        grantee: PublicKey,
        data_id: String,
        locked_until: Option<EpochHeight>,
    ) {
        let owner = env::predecessor_account_id();

        self.find_grants(
            Some(owner.clone()),
            Some(grantee.clone()),
            Some(data_id.clone()),
        )
        .iter()
        .filter(|grant| match locked_until {
            None => true,
            Some(0) => true,
            Some(locked_until_) => grant.locked_until == locked_until_,
        })
        .for_each(|grant| {
            require!(
                grant.locked_until < env::block_timestamp(),
                "Grant is timelocked"
            );

            let grant_id = derive_grant_id(grant);

            self.grants_by_id.remove(&grant_id);

            remove_values(&mut self.grant_ids_by_owner, &owner, &grant_id);
            remove_values(&mut self.grant_ids_by_grantee, &grantee, &grant_id);
            remove_values(&mut self.grant_ids_by_data_id, &data_id, &grant_id);
        });

        env::log_str(&format!(
            "EVENT_JSON:{}",
            json!({
                "standard": "FractalRegistry",
                "version": "0",
                "event": "grant_deleted",
                "data": {
                    "owner": owner,
                    "grantee": grantee,
                    "data_id": data_id,
                    "locked_until": locked_until.unwrap_or(0),
                },
            })
        ))
    }

    pub fn grants_for(&self, grantee: PublicKey, data_id: String) -> Vec<Grant> {
        self.find_grants(None, Some(grantee), Some(data_id))
    }

    pub fn find_grants(
        &self,
        owner: Option<AccountId>,
        grantee: Option<PublicKey>,
        data_id: Option<String>,
    ) -> Vec<Grant> {
        let mut grant_id_searches = Vec::new();

        require!(
            owner.is_some() || grantee.is_some(),
            "Required argument: `owner` and/or `grantee`",
        );

        if let Some(owner) = owner {
            grant_id_searches.push(self.grant_ids_by_owner.get(&owner).unwrap_or_default());
        }

        if let Some(grantee) = grantee {
            grant_id_searches.push(self.grant_ids_by_grantee.get(&grantee).unwrap_or_default());
        }

        if let Some(data_id) = data_id {
            grant_id_searches.push(self.grant_ids_by_data_id.get(&data_id).unwrap_or_default());
        }

        let Some((head, tail)) = grant_id_searches.split_first() else {
            return vec![];
        };

        head.iter()
            .filter(|id| tail.iter().all(|s| s.contains(id)))
            .map(|id| self.grants_by_id.get(id).unwrap())
            .collect()
    }
}
