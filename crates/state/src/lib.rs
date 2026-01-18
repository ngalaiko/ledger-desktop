pub mod period;

use std::collections::HashSet;

use anyhow::{anyhow, Error};
use chrono::Datelike;
use gpui::{App, AppContext, Context, Entity, Global, Subscription, Task};
use ledger::Account;
use period::Period;

pub fn init(cx: &mut App) {
    AppState::set_global(cx.new(AppState::new), cx);
}

macro_rules! setting_accessors {
    ($(pub $field:ident: $type:ty),* $(,)?) => {
        impl AppState {
            $(
                paste::paste! {
                    pub fn [<get_ $field>](cx: &App) -> $type {
                        Self::global(cx).read(cx).values.$field.clone()
                    }

                    pub fn [<update_ $field>](value: $type, cx: &mut App) {
                        Self::global(cx).update(cx, |this, cx| {
                            this.values.$field = value;
                            cx.notify();
                        });
                    }
                }
            )*
        }
    };
}

setting_accessors! {
    pub commodity: Option<String>,
    pub selected_accounts: HashSet<Account>,
    pub expanded_accounts: HashSet<Account>,
    pub period: Period,
    pub period_idx: usize,
    pub selected_total_assets_tab_idx: usize,
    pub selected_expenses_tab_idx: usize,
    pub selected_revenue_tab_idx: usize,
}

static CURRENT_VERSION: &str = "1.0";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub version: String,
    pub commodity: Option<String>,
    pub selected_accounts: HashSet<Account>,
    pub expanded_accounts: HashSet<Account>,
    pub period: Period,
    // index of the current period. 0 = current, 1 = previous, 2 = two periods ago, etc.
    pub period_idx: usize,
    pub selected_total_assets_tab_idx: usize,
    #[serde(default)]
    pub selected_expenses_tab_idx: usize,
    #[serde(default)]
    pub selected_revenue_tab_idx: usize,
}

impl State {
    pub fn get_period_interval(&self) -> (chrono::NaiveDate, chrono::NaiveDate) {
        let today_date = chrono::Local::now().date_naive();
        let interval_end = match self.period {
            Period::Week => {
                today_date
                    - chrono::Duration::days(today_date.weekday().num_days_from_monday() as i64)
                    - chrono::Duration::weeks(self.period_idx as i64)
            }
            Period::Month => {
                let mut year = today_date.year();
                let mut month = today_date.month() as i32 - self.period_idx as i32;
                while month <= 0 {
                    month += 12;
                    year -= 1;
                }
                chrono::NaiveDate::from_ymd_opt(year, month as u32, 1)
                    .unwrap()
                    .checked_sub_signed(chrono::Duration::days(1))
                    .expect("valid date")
            }
            Period::Year => {
                let year = today_date.year() - self.period_idx as i32;
                chrono::NaiveDate::from_ymd_opt(year, 12, 31).expect("valid date")
            }
        };
        let interval_start = match self.period {
            Period::Week => interval_end + chrono::Duration::days(1) - chrono::Duration::weeks(1),
            Period::Month => {
                let mut year = interval_end.year();
                let mut month = interval_end.month() as i32;
                while month <= 1 {
                    month += 12;
                    year -= 1;
                }
                chrono::NaiveDate::from_ymd_opt(year, month as u32, 1).expect("valid date")
            }
            Period::Year => {
                chrono::NaiveDate::from_ymd_opt(interval_end.year(), 1, 1).expect("valid date")
            }
        };
        (interval_start, interval_end)
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION.to_string(),
            commodity: None,
            selected_accounts: HashSet::new(),
            expanded_accounts: HashSet::new(),
            period: Period::Month,
            period_idx: 0,
            selected_total_assets_tab_idx: 0,
            selected_expenses_tab_idx: 0,
            selected_revenue_tab_idx: 0,
        }
    }
}

impl AsRef<State> for State {
    fn as_ref(&self) -> &State {
        self
    }
}

struct GlobalAppState(Entity<AppState>);

impl Global for GlobalAppState {}

pub struct AppState {
    pub values: State,
    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl AppState {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAppState>().0.clone()
    }

    pub(crate) fn set_global(state: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalAppState(state));
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let load_state = Self::load_state(cx);

        let mut tasks = vec![];
        let mut subscriptions = vec![];

        subscriptions.push(
            // observe and automatically save state on change
            cx.observe_self(|this, cx| {
                this.save_state(cx);
            }),
        );

        tasks.push(
            // load the initial settings
            cx.spawn(async move |this, cx| {
                if let Ok(state) = load_state.await {
                    this.update(cx, |this, cx| {
                        this.values = state;
                        cx.notify();
                    })
                    .ok();
                }
            }),
        );

        Self {
            values: State::default(),
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }

    fn save_state(&self, cx: &App) {
        if let Ok(state) = serde_json::to_vec(&self.values) {
            let task: Task<Result<(), Error>> = cx.background_spawn(async move {
                let config_dir =
                    dirs::config_dir().ok_or(anyhow!("could not determine config directory"))?;
                let app_config_dir = config_dir.join("ledger-desktop");
                async_fs::create_dir_all(&app_config_dir).await?;

                let config_file = app_config_dir.join("state.json");
                async_fs::write(&config_file, state).await?;

                Ok(())
            });

            task.detach()
        }
    }

    fn load_state(cx: &App) -> Task<Result<State, Error>> {
        cx.background_spawn(async move {
            let config_dir =
                dirs::config_dir().ok_or(anyhow!("could not determine config directory"))?;
            let config_file = config_dir.join("ledger-desktop").join("state.json");

            if config_file.exists() {
                let data = async_fs::read(&config_file).await?;
                let state: State = serde_json::from_slice(&data)?;
                if state.version != CURRENT_VERSION {
                    Ok(State::default())
                } else {
                    Ok(state)
                }
            } else {
                Ok(State::default())
            }
        })
    }

    pub fn get_period_interval(cx: &App) -> (chrono::NaiveDate, chrono::NaiveDate) {
        Self::global(cx).read(cx).values.get_period_interval()
    }

    pub fn update_period_today(cx: &mut App) {
        Self::global(cx).update(cx, |this, cx| {
            this.values.period_idx = 0;
            cx.notify();
        });
    }

    pub fn update_period_next(cx: &mut App) {
        Self::global(cx).update(cx, |this, cx| {
            this.values.period_idx -= 1;
            cx.notify();
        });
    }

    pub fn update_period_prev(cx: &mut App) {
        Self::global(cx).update(cx, |this, cx| {
            this.values.period_idx += 1;
            cx.notify();
        });
    }
}
