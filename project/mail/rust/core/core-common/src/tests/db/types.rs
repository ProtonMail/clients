use crate::datatypes::{AccountDetails, AuthScopes, AvatarInformation};
use crate::db::account::{CoreAccount, CoreSession};
use crate::db::migrations::migrate_account_db;
use mail_api_session::ids::SessionId;
use mail_core_api::services::proton::UserId;
use mail_shared_types::ModelExtension;
use mail_stash::orm::Model;
use mail_stash::stash::{Stash, StashConfiguration};
use std::default::Default;

impl Default for CoreAccount {
    fn default() -> Self {
        Self::new(UserId::new("__NOT_USED__".to_string()), String::new())
    }
}

#[cfg(test)]
mod core_account_details_tests {
    use super::*;

    #[test]
    fn test_all_fields_present() {
        let sut = CoreAccount {
            name_or_addr: "frank.moon@pm.me".to_string(),
            display_name: Some("Frankie".to_string()),
            username: Some("Frank Moon".to_string()),
            primary_addr: Some("frank@proton.me".to_string()),
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "Frankie", "frank@proton.me");
    }

    #[test]
    fn test_no_display_name_fallback_to_username() {
        let sut = CoreAccount {
            name_or_addr: "Max Johnson".to_string(),
            display_name: None,
            username: Some("Max".to_string()),
            primary_addr: Some("max@pm.me".to_string()),
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "Max", "max@pm.me");
    }

    #[test]
    fn test_no_display_name_or_username_fallback_to_name_or_addr() {
        let sut = CoreAccount {
            name_or_addr: "John Doe".to_string(),
            display_name: None,
            username: None,
            primary_addr: Some("john@gmail.com".to_string()),
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "John Doe", "john@gmail.com");
    }

    #[test]
    fn test_no_primary_addr_fallback_to_name_or_addr() {
        let sut = CoreAccount {
            name_or_addr: "dricus@proton.me".to_string(),
            display_name: Some("Dricus".to_string()),
            username: Some("Dricus Du Plessis".to_string()),
            primary_addr: None,
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "Dricus", "dricus@proton.me");
    }

    #[test]
    fn test_blank_display_name_fallback_to_username() {
        let sut = CoreAccount {
            name_or_addr: "dricus@proton.me".to_string(),
            display_name: Some(String::new()),
            username: Some("Dricus Du Plessis".to_string()),
            primary_addr: None,
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "Dricus Du Plessis", "dricus@proton.me");
    }

    #[test]
    fn test_blank_display_name_or_username_fallback_to_name_or_addr() {
        let sut = CoreAccount {
            name_or_addr: "dricus@proton.me".to_string(),
            display_name: Some(String::new()),
            username: Some(String::new()),
            primary_addr: None,
            ..Default::default()
        };

        let result = sut.details();

        assert_account_details(&result, "dricus@proton.me", "dricus@proton.me");
    }

    fn assert_account_details(result: &AccountDetails, expected_name: &str, expected_email: &str) {
        assert_eq!(result.name, expected_name);
        assert_eq!(result.email, expected_email);
        assert_eq!(
            result.avatar_information,
            AvatarInformation::from(expected_name)
        );
    }
}

#[tokio::test]
#[allow(
    invalid_value,
    reason = "We are only testing db data writes, zeroed out vec is just empty"
)]
async fn core_session_save_deletes_previous_session_for_same_account_id() {
    let stash = Stash::new(StashConfiguration::test()).unwrap();
    migrate_account_db(&stash).await.unwrap();

    let user_id = UserId::from("USER");
    let mut account = CoreAccount {
        remote_id: user_id.clone(),
        name_or_addr: "dricus@proton.me".to_string(),
        display_name: Some(String::new()),
        username: Some(String::new()),
        primary_addr: None,
        ..Default::default()
    };

    let session_id1 = SessionId::from("SESSION1");
    let session_id2 = SessionId::from("SESSION2");

    let mut session1 = CoreSession {
        remote_id: session_id1.clone(),
        account_id: user_id.clone(),
        access_token: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
        refresh_token: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
        auth_scopes: AuthScopes::new(["full"]),
        key_secret: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
    };

    let mut session2 = CoreSession {
        remote_id: session_id2.clone(),
        account_id: user_id.clone(),
        access_token: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
        refresh_token: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
        auth_scopes: AuthScopes::new(["full"]),
        key_secret: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
    };

    let mut tether = stash.connection();

    tether
        .write_tx(async |tx| {
            account.save(tx).await?;
            session1.save(tx).await?;
            session2.save(tx).await
        })
        .await
        .unwrap();

    assert!(
        CoreSession::find_by_id(session_id1, &tether)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        CoreSession::find_by_id(session_id2, &tether)
            .await
            .unwrap()
            .is_some()
    );
}
