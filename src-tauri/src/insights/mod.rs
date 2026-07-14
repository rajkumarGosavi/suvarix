//! Behavioural nudges ("Smart Insights") — a prioritised, actionable feed derived
//! entirely from data the app already holds (health pillars, budgets, maturities,
//! net-worth history). No new data collection. Core feature (not gamification-gated).
//!
//! Each nudge is "one number + one action": a short status and a single button that
//! routes to the screen where the user fixes it. The same list feeds two surfaces:
//! the in-app Dashboard feed (`get_insights`) and the background scheduler, which
//! pushes the urgent ones as native notifications (deduped weekly).

pub mod commands;
