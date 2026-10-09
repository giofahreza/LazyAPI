//! Coalesce background updates by request identity, even when its tab is closed.
use std::sync::{Arc, Weak};
use parking_lot::{Mutex, RwLock};
use crate::models::request::Request;

pub type ResponseUpdates = Arc<Mutex<Vec<Weak<RwLock<Request>>>>>;

pub fn notify_response(updates: &ResponseUpdates, request: &Arc<RwLock<Request>>) {
    let request = Arc::downgrade(request);
    let mut pending = updates.lock();
    if !pending.iter().any(|item| item.ptr_eq(&request)) { pending.push(request); }
}
