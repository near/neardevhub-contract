use near_sdk::near;

pub type TimelineStatus = TimelineStatusV2;
type ReviewStatus = ReviewStatusV2;

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimelineStatusV1 {
    Draft,
    Review(ReviewStatusV1),
    Approved(ReviewStatusV1),
    Rejected(ReviewStatusV1),
    ApprovedConditionally(ReviewStatusV1),
    PaymentProcessing(PaymentProcessingStatusV1),
    Funded(FundedStatusV1),
    Cancelled(ReviewStatusV1),
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
#[serde(tag = "timeline_version")]
pub enum VersionedTimelineStatus {
    V1(TimelineStatusV2),
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimelineStatusV2 {
    Draft,
    Review(ReviewStatusV2),
    Approved(ReviewStatusV2),
    Rejected(ReviewStatusV2),
    ApprovedConditionally(ReviewStatusV2),
    PaymentProcessing(PaymentProcessingStatusV2),
    Funded(FundedStatusV2),
    Cancelled(ReviewStatusV2),
}

fn convert_review_status_to_v1(
    review_status: ReviewStatusV1,
    kyc_verified: bool,
) -> ReviewStatusV2 {
    ReviewStatusV2 {
        sponsor_requested_review: review_status.sponsor_requested_review,
        reviewer_completed_attestation: review_status.reviewer_completed_attestation,
        kyc_verified: kyc_verified,
    }
}

impl From<PaymentProcessingStatusV1> for PaymentProcessingStatusV2 {
    fn from(value: PaymentProcessingStatusV1) -> Self {
        PaymentProcessingStatusV2 {
            review_status: convert_review_status_to_v1(value.review_status, value.kyc_verified),
            kyc_verified_deprecated: false,
            test_transaction_sent: value.test_transaction_sent,
            request_for_trustees_created: value.request_for_trustees_created,
        }
    }
}

impl From<FundedStatusV1> for FundedStatusV2 {
    fn from(value: FundedStatusV1) -> Self {
        FundedStatusV2 {
            payment_processing_status: value.payment_processing_status.into(),
            trustees_released_payment: value.trustees_released_payment,
            payouts: value.payouts,
        }
    }
}

impl From<TimelineStatusV1> for TimelineStatusV2 {
    fn from(value: TimelineStatusV1) -> Self {
        match value {
            TimelineStatusV1::Draft => TimelineStatusV2::Draft,
            TimelineStatusV1::Review(review_status) => {
                TimelineStatusV2::Review(convert_review_status_to_v1(review_status, false))
            }
            TimelineStatusV1::Approved(review_status) => {
                TimelineStatusV2::Approved(convert_review_status_to_v1(review_status, false))
            }
            TimelineStatusV1::Rejected(review_status) => {
                TimelineStatusV2::Rejected(convert_review_status_to_v1(review_status, false))
            }
            TimelineStatusV1::ApprovedConditionally(review_status) => {
                TimelineStatusV2::ApprovedConditionally(convert_review_status_to_v1(
                    review_status,
                    false,
                ))
            }
            TimelineStatusV1::PaymentProcessing(payment_processing_status) => {
                TimelineStatusV2::PaymentProcessing(payment_processing_status.into())
            }
            TimelineStatusV1::Funded(funded_status) => {
                TimelineStatusV2::Funded(funded_status.into())
            }
            TimelineStatusV1::Cancelled(review_status) => {
                TimelineStatusV2::Cancelled(convert_review_status_to_v1(review_status, false))
            }
        }
    }
}

impl VersionedTimelineStatus {
    pub fn latest_version(self) -> TimelineStatus {
        self.into()
    }
}

impl From<VersionedTimelineStatus> for TimelineStatusV2 {
    fn from(value: VersionedTimelineStatus) -> Self {
        match value {
            VersionedTimelineStatus::V1(v1) => v1,
        }
    }
}

impl From<TimelineStatusV2> for VersionedTimelineStatus {
    fn from(value: TimelineStatusV2) -> Self {
        VersionedTimelineStatus::V1(value)
    }
}

impl From<TimelineStatusV1> for VersionedTimelineStatus {
    fn from(value: TimelineStatusV1) -> Self {
        VersionedTimelineStatus::V1(value.into())
    }
}

impl TimelineStatus {
    pub fn is_draft(&self) -> bool {
        matches!(self, TimelineStatus::Draft)
    }

    pub fn is_empty_review(&self) -> bool {
        match self {
            TimelineStatus::Review(review_status) => {
                !review_status.sponsor_requested_review
                    && !review_status.reviewer_completed_attestation
            }
            _ => false,
        }
    }

    pub fn is_review(&self) -> bool {
        matches!(self, TimelineStatus::Review(..))
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, TimelineStatus::Cancelled(..))
    }

    pub fn can_be_cancelled(&self) -> bool {
        match self {
            TimelineStatus::Draft => true,
            TimelineStatus::Review(..) => true,
            _ => false,
        }
    }

    pub fn was_approved(&self) -> bool {
        match self {
            TimelineStatus::Approved(..) => true,
            TimelineStatus::ApprovedConditionally(..) => true,
            TimelineStatus::PaymentProcessing(..) => true,
            TimelineStatus::Funded(..) => true,
            _ => false,
        }
    }

    pub fn get_review_status(&self) -> &ReviewStatus {
        match self {
            TimelineStatus::Review(review_status)
            | TimelineStatus::Approved(review_status)
            | TimelineStatus::Rejected(review_status)
            | TimelineStatus::ApprovedConditionally(review_status)
            | TimelineStatus::Cancelled(review_status) => review_status.into(),
            TimelineStatus::PaymentProcessing(payment_processing_status) => {
                &payment_processing_status.review_status
            }
            TimelineStatus::Funded(funded_status) => {
                &funded_status.payment_processing_status.review_status
            }
            TimelineStatus::Draft => &ReviewStatus {
                sponsor_requested_review: false,
                reviewer_completed_attestation: false,
                kyc_verified: false,
            },
        }
    }
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct ReviewStatusV1 {
    sponsor_requested_review: bool,
    reviewer_completed_attestation: bool,
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct ReviewStatusV2 {
    sponsor_requested_review: bool,
    reviewer_completed_attestation: bool,
    kyc_verified: bool,
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct PaymentProcessingStatusV1 {
    #[serde(flatten)]
    review_status: ReviewStatusV1,
    kyc_verified: bool,
    test_transaction_sent: bool,
    request_for_trustees_created: bool,
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct PaymentProcessingStatusV2 {
    #[serde(flatten)]
    review_status: ReviewStatusV2,
    #[serde(default)]
    kyc_verified_deprecated: bool,
    test_transaction_sent: bool,
    request_for_trustees_created: bool,
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct FundedStatusV1 {
    #[serde(flatten)]
    payment_processing_status: PaymentProcessingStatusV1,
    trustees_released_payment: bool,
    payouts: Vec<String>,
}

#[near(serializers=[borsh, json])]
#[derive(Clone, Debug)]
pub struct FundedStatusV2 {
    #[serde(flatten)]
    payment_processing_status: PaymentProcessingStatusV2,
    trustees_released_payment: bool,
    payouts: Vec<String>,
}
