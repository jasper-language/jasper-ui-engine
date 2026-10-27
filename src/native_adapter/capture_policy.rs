//! Pure cancellation admission; native delivery and host release remain separate effects.
use jasper_ui_engine::input_identity::{UiInputContext,accepts_ui_display_snapshot,matches_ui_view_owner};
// BEGIN VERIFIED CAPTURE CANCEL ROUTE TYPE
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum CaptureCancelRoute {Reject,HostOnly,Current,Successor}
// END VERIFIED CAPTURE CANCEL ROUTE TYPE
// BEGIN VERIFIED CAPTURE CANCEL POLICY KERNEL
pub(crate) fn capture_cancel_route(session:u64,current:UiInputContext,caller:u64,origin:UiInputContext,deliver:bool,open:bool)->CaptureCancelRoute {
    if !accepts_ui_display_snapshot(session,origin,caller,origin,current) {CaptureCancelRoute::Reject}
    else if !deliver || !open {CaptureCancelRoute::HostOnly}
    else if matches_ui_view_owner(session,current,caller,origin) {CaptureCancelRoute::Current}
    else {CaptureCancelRoute::Successor}
}
// END VERIFIED CAPTURE CANCEL POLICY KERNEL
