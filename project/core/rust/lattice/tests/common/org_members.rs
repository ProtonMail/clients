//! Org member lookup helpers for integration tests.

use lattice::core::get_members::LtCoreMemberInfo;

pub use super::org_member_error::OrgMemberError;

pub fn find_member_by_email<'a>(
    members: &'a [LtCoreMemberInfo],
    email: &str,
) -> Result<&'a LtCoreMemberInfo, OrgMemberError> {
    members
        .iter()
        .find(|m| m.name == email)
        .ok_or(OrgMemberError::MemberNotFound {
            email: email.to_string(),
            num_members: members.len(),
        })
}
