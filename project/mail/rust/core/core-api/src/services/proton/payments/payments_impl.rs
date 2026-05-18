use crate::service::ApiServiceResult;
use crate::services::proton::payments::{PAYMENTS_V5, ProtonPayments};
use crate::services::proton::prelude::*;
use bytes::Bytes;
use mail_muon::common::Sender;
use mail_muon::util::ProtonRequestExt;
use mail_muon::{GET, POST, ProtonRequest, ProtonResponse, serde_to_query};

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonPayments for This {
    async fn get_payments_status(
        &self,
        vendor: String,
    ) -> ApiServiceResult<GetPaymentsStatusResponse> {
        Ok(GET!("{PAYMENTS_V6}/status/{vendor}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_payments_plans(
        &self,
        options: GetPaymentsPlansOptions,
    ) -> ApiServiceResult<GetPaymentsPlansResponse> {
        Ok(GET!("{PAYMENTS_V5}/plans")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_payments_resources_icons(&self, name: String) -> ApiServiceResult<Bytes> {
        Ok(GET!("{PAYMENTS_V5}/resources/icons/{name}")
            .send_with(self)
            .await?
            .ok()?
            .into_body()
            .into())
    }

    async fn post_payments_tokens(
        &self,
        amount: u64,
        currency: String,
        payment: PaymentReceipt,
    ) -> ApiServiceResult<PostPaymentsTokensResponse> {
        Ok(POST!("{PAYMENTS_V5}/tokens")
            .body_json(PostPaymentsTokensRequest {
                amount,
                currency,
                payment,
            })?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_payments_subscription(&self) -> ApiServiceResult<GetPaymentsSubscriptionResponse> {
        Ok(GET!("{PAYMENTS_V5}/subscription")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn post_payments_subscription(
        &self,
        subscription: NewSubscription,
        new_values: NewSubscriptionValues,
    ) -> ApiServiceResult<()> {
        POST!("{PAYMENTS_V5}/subscription")
            .body_json(PostPaymentsSubscriptionRequest {
                subscription,
                new_values,
            })?
            .send_with(self)
            .await?
            .ok()?;

        Ok(())
    }

    async fn get_payment_method(
        &self,
        payment_method_id: String,
    ) -> ApiServiceResult<GetPaymentMethodResponse> {
        Ok(GET!("{PAYMENTS_V5}/methods/{payment_method_id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
