use super::{ApplicationState, InterestState, PreparationState, Queue};

pub(super) const ARCHIVE: &str = "(interest_state = 'dismissed' OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))";
pub(super) const APPLIED: &str = "(NOT (interest_state = 'dismissed' OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted')) AND application_state IN ('submitted', 'in_process'))";
pub(super) const INBOX: &str = "(NOT (interest_state = 'dismissed' OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted')) AND interest_state = 'undecided' AND application_state = 'not_applied')";
pub(super) const NEW_INBOX: &str = "(NOT (interest_state = 'dismissed' OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted')) AND read_state = 'unread' AND interest_state = 'undecided' AND application_state = 'not_applied')";
pub(super) const REVIEW_INBOX: &str = "(NOT (interest_state = 'dismissed' OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted')) AND read_state = 'read' AND interest_state = 'undecided' AND application_state = 'not_applied')";
pub(super) const INTERESTED: &str = "(interest_state = 'interested' AND preparation_state = 'not_started' AND application_state = 'not_applied')";
pub(super) const PREPARATION: &str = "(interest_state = 'interested' AND application_state = 'not_applied' AND preparation_state IN ('in_progress', 'ready'))";

pub(super) fn condition(queue: Queue) -> Option<&'static str> {
    match queue {
        Queue::All => None,
        Queue::Archive => Some(ARCHIVE),
        Queue::Applied => Some(APPLIED),
        Queue::Inbox => Some(INBOX),
        Queue::Interested => Some(INTERESTED),
        Queue::Preparation => Some(PREPARATION),
    }
}

pub(super) fn primary(
    interest: InterestState,
    preparation: PreparationState,
    application: ApplicationState,
) -> Queue {
    if interest == InterestState::Dismissed
        || matches!(
            application,
            ApplicationState::RejectedByCompany
                | ApplicationState::WithdrawnByMe
                | ApplicationState::Accepted
        )
    {
        Queue::Archive
    } else if matches!(
        application,
        ApplicationState::Submitted | ApplicationState::InProcess
    ) {
        Queue::Applied
    } else if interest == InterestState::Undecided && application == ApplicationState::NotApplied {
        Queue::Inbox
    } else if interest == InterestState::Interested
        && application == ApplicationState::NotApplied
        && matches!(
            preparation,
            PreparationState::InProgress | PreparationState::Ready
        )
    {
        Queue::Preparation
    } else {
        Queue::Interested
    }
}
