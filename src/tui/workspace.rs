//! Request tabs and spatial pane focus. Tabs hold request identities, never file indexes.
use std::sync::Arc;
use parking_lot::RwLock;
use crate::app::app::App;
use crate::models::request::Request;
use crate::tui::app_states::AppState;
use crate::tui::ui::param_tabs::param_tabs::RequestParamsTabs;
use crate::tui::ui::result_tabs::RequestResultTabs;
use crate::tui::ui::views::RequestView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Collections,
    Url,
    Parameters,
    Response,
}

impl Pane {
    pub fn label(self) -> &'static str {
        match self {
            Self::Collections => "Collections",
            Self::Url => "URL",
            Self::Parameters => "Parameters",
            Self::Response => "Response",
        }
    }
}

pub struct RequestTab {
    pub request: Arc<RwLock<Request>>,
    pub pane: Pane,
    pub view: RequestView,
    pub params: RequestParamsTabs,
    pub result: RequestResultTabs,
    pub query_selection: Option<(usize, usize)>,
    pub header_selection: Option<(usize, usize)>,
    pub body_selection: Option<(usize, usize)>,
    pub auth_selection: usize,
    pub script_selection: u16,
    pub scroll: (u16, u16),
}

impl RequestTab {
    fn new(request: Arc<RwLock<Request>>) -> Self {
        Self {
            request, pane: Pane::Parameters, view: RequestView::Normal,
            params: RequestParamsTabs::QueryParams, result: RequestResultTabs::Body,
            query_selection: None, header_selection: None, body_selection: None,
            auth_selection: 0, script_selection: 0, scroll: (0, 0),
        }
    }
}

impl App<'_> {
    pub fn request_location(&self, request: &Arc<RwLock<Request>>) -> Option<(usize, usize)> {
        self.collections.iter().enumerate().find_map(|(c, collection)| {
            collection.requests.iter().position(|r| Arc::ptr_eq(r, request)).map(|r| (c, r))
        })
    }

    pub fn remember_request_tab(&mut self) {
        if self.collections_tree.selected.is_none() { return; }
        if let Some(tab) = self.active_request_tab.and_then(|i| self.request_tabs.get_mut(i)) {
            if self.pane != Pane::Collections { tab.pane = self.pane; }
            tab.view = self.request_view;
            tab.params = self.request_param_tab;
            tab.result = self.request_result_tab;
            tab.query_selection = self.query_params_table.selection;
            tab.header_selection = self.headers_table.selection;
            tab.body_selection = self.body_form_table.selection;
            tab.auth_selection = self.auth_text_input_selection.selected;
            tab.script_selection = self.script_console.script_selection;
            tab.scroll = (self.result_vertical_scrollbar.scroll, self.result_horizontal_scrollbar.scroll);
        }
    }

    pub fn open_request_tab(&mut self, request: Arc<RwLock<Request>>) {
        self.remember_request_tab();
        let index = self.request_tabs.iter().position(|tab| Arc::ptr_eq(&tab.request, &request))
            .unwrap_or_else(|| {
                self.request_tabs.push(RequestTab::new(request));
                self.request_tabs.len() - 1
            });
        self.activate_request_tab(index);
    }

    pub fn activate_request_tab(&mut self, index: usize) {
        let Some(tab) = self.request_tabs.get(index) else { return; };
        let Some(location) = self.request_location(&tab.request) else { return; };
        self.active_request_tab = Some(index);
        self.collections_tree.selected = Some(location);
        self.request_view = tab.view;
        self.request_param_tab = tab.params;
        self.request_result_tab = tab.result;
        self.pane = tab.pane;
        self.query_params_table.update_selection(tab.query_selection);
        self.headers_table.update_selection(tab.header_selection);
        self.body_form_table.update_selection(tab.body_selection);
        self.auth_text_input_selection.selected = tab.auth_selection;
        self.script_console.script_selection = tab.script_selection;
        self.result_vertical_scrollbar.scroll = tab.scroll.0;
        self.result_horizontal_scrollbar.scroll = tab.scroll.1;
        self.tui_update_request_param_tab();
        self.tui_update_request_result_tab();
        self.select_request_state();
        self.refresh_active_response();
        self.open_focused_body_editor();
    }

    pub fn switch_request_tab(&mut self, forward: bool) {
        if self.request_tabs.is_empty() { return; }
        self.remember_request_tab();
        let current = self.active_request_tab.unwrap_or(0);
        let count = self.request_tabs.len();
        let index = if forward { (current + 1) % count } else { (current + count - 1) % count };
        self.activate_request_tab(index);
    }

    pub fn close_request_tab(&mut self) {
        let Some(index) = self.active_request_tab else { return; };
        self.request_tabs.remove(index);
        self.active_request_tab = None;
        self.collections_tree.selected = None;
        if self.request_tabs.is_empty() {
            self.pane = Pane::Collections;
            self.state = AppState::Normal;
            self.was_last_state_selected_request = false;
        } else {
            self.activate_request_tab(index.min(self.request_tabs.len() - 1));
        }
    }

    /// Keep identities valid after deleting or reordering collections/requests.
    pub fn sync_request_tabs(&mut self) {
        let current = self.active_request_tab.and_then(|i| self.request_tabs.get(i)).map(|t| t.request.clone());
        let collections = &self.collections;
        self.request_tabs.retain(|tab| collections.iter().any(|c| c.requests.iter().any(|r| Arc::ptr_eq(r, &tab.request))));
        if let Some(request) = current {
            if let Some(index) = self.request_tabs.iter().position(|t| Arc::ptr_eq(&t.request, &request)) {
                self.active_request_tab = Some(index);
                self.collections_tree.selected = self.request_location(&request);
            } else {
                let index = self.active_request_tab.unwrap_or(0).min(self.request_tabs.len().saturating_sub(1));
                self.active_request_tab = None;
                self.collections_tree.selected = None;
                if self.request_tabs.is_empty() {
                    self.pane = Pane::Collections;
                    self.state = AppState::Normal;
                } else {
                    self.activate_request_tab(index);
                    self.focus_pane(Pane::Collections);
                }
            }
        }
    }

    pub fn focus_pane(&mut self, pane: Pane) {
        if pane != Pane::Collections && self.collections_tree.selected.is_none() { return; }
        if pane == Pane::Collections && self.pane != pane { self.remember_request_tab(); }
        self.pane = pane;
        self.state = if pane == Pane::Collections { AppState::Normal } else { AppState::SelectedRequest };
        if (pane == Pane::Parameters && self.request_view == RequestView::OnlyResult)
            || (pane == Pane::Response && self.request_view == RequestView::OnlyParams) {
            self.request_view = RequestView::Normal;
        }
        self.open_focused_body_editor();
    }

    pub fn pane_in_direction(&self, direction: char) -> Pane {
        use Pane::*;
        match (self.pane, direction) {
            (Collections, 'l') => Parameters,
            (Parameters | Url, 'h') => Collections,
            (Response, 'h') => Parameters,
            (Parameters, 'l') => Response,
            (Parameters | Response, 'k') => Url,
            (Url, 'j') => Parameters,
            _ => self.pane,
        }
    }

    pub fn refresh_active_response(&mut self) {
        if self.collections_tree.selected.is_some() {
            self.tui_highlight_response_body_and_console();
            self.tui_refresh_result_scrollbars();
        }
    }

    pub fn previous_request_panel(&mut self) {
        // Advance through the finite, protocol-specific cycle to find its predecessor.
        if self.pane == Pane::Response {
            let original = self.request_result_tab;
            let mut previous = original;
            for _ in 0..5 {
                self.tui_next_request_result_tab();
                if self.request_result_tab == original { break; }
                previous = self.request_result_tab;
            }
            self.request_result_tab = previous;
            self.refresh_active_response();
        } else if self.pane == Pane::Parameters {
            for _ in 0..4 { self.tui_next_request_param_tab(); }
        }
    }
}
