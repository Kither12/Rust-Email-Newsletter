use crate::helpers::{spawn_app, TestApp};
use rust_email_newsletter::email_client::{ConfirmationLink, EmailClient};

async fn create_unconfirm_subscriber(app: &TestApp) -> ConfirmationLink {
    let body = "name=testName&email=testEmail%40gmail.com";
    let response = app
        .post_subscriptions(body)
        .await
        .error_for_status()
        .expect("Failed to create subscriber");
    let subscription_token = response.text().await.expect("Failed to get response");
    EmailClient::get_confirmation_link(&app.address, &subscription_token)
}
async fn create_confirm_subscriber(app: &TestApp) {
    let confimation_link = create_unconfirm_subscriber(app).await;
    let client = reqwest::Client::new();
    client
        .get(confimation_link.0)
        .send()
        .await
        .expect("Failed to execute request");
}

#[tokio::test]
async fn newsletter_are_delivered_to_confirmed_subscriber() {
    let app = spawn_app().await;
    create_confirm_subscriber(&app).await;
    let newsletter_request_body = serde_json::json!({
        "subject": "Newsletter title",
        "content": "<p>Newsletter body as HTML</p>",
    });
    let response = app.post_newsletter(newsletter_request_body).await;
    assert_eq!(response.status().as_u16(), 200);
}
#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "text": "Newsletter body as plain text",
            }),
            "missing title",
        ),
        (
            serde_json::json!({"subject": "Newsletter!"}),
            "missing content",
        ),
    ];
    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletter(invalid_body).await;
        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
