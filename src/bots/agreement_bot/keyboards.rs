use notifine::i18n::t;
use notifine::models::Agreement;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

pub fn build_disclaimer_keyboard(language: &str) -> InlineKeyboardMarkup {
    let accept_text = t(language, "agreement.disclaimer.accept_button");
    let decline_text = t(language, "agreement.disclaimer.decline_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(accept_text, "disclaimer:accept"),
        InlineKeyboardButton::callback(decline_text, "disclaimer:decline"),
    ]])
}

pub fn build_menu_keyboard(language: &str) -> InlineKeyboardMarkup {
    let rent_text = t(language, "agreement.menu.rent_button");
    let custom_text = t(language, "agreement.menu.custom_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(rent_text, "menu:rent"),
        InlineKeyboardButton::callback(custom_text, "menu:custom"),
    ]])
}

pub fn build_language_keyboard(language: &str) -> InlineKeyboardMarkup {
    let en_text = t(language, "agreement.language.en_button");
    let tr_text = t(language, "agreement.language.tr_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(en_text, "lang:en"),
        InlineKeyboardButton::callback(tr_text, "lang:tr"),
    ]])
}

pub fn build_timezone_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Europe/Istanbul",
            "tz:Europe/Istanbul",
        )],
        vec![InlineKeyboardButton::callback(
            "Europe/London",
            "tz:Europe/London",
        )],
        vec![InlineKeyboardButton::callback(
            "America/New_York",
            "tz:America/New_York",
        )],
    ])
}

pub fn build_settings_keyboard(language: &str) -> InlineKeyboardMarkup {
    let language_text = t(language, "agreement.settings.change_language");
    let timezone_text = t(language, "agreement.settings.change_timezone");

    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            language_text,
            "settings:language",
        )],
        vec![InlineKeyboardButton::callback(
            timezone_text,
            "settings:timezone",
        )],
    ])
}

#[allow(dead_code)]
pub fn build_cancel_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        t(language, "common.cancel_button"),
        "flow:cancel",
    )]])
}

pub fn build_yes_no_keyboard(language: &str, callback_prefix: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "common.yes_button"),
            format!("{}:yes", callback_prefix),
        ),
        InlineKeyboardButton::callback(
            t(language, "common.no_button"),
            format!("{}:no", callback_prefix),
        ),
    ]])
}

#[allow(dead_code)]
pub fn build_role_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.tenant_button"),
            "rent:role:tenant",
        ),
        InlineKeyboardButton::callback(
            t(language, "agreement.rent.step2_role.landlord_button"),
            "rent:role:landlord",
        ),
    ]])
}

pub fn build_currency_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🇹🇷 TRY", "rent:currency:TRY"),
            InlineKeyboardButton::callback("🇪🇺 EUR", "rent:currency:EUR"),
        ],
        vec![
            InlineKeyboardButton::callback("🇺🇸 USD", "rent:currency:USD"),
            InlineKeyboardButton::callback("🇬🇧 GBP", "rent:currency:GBP"),
        ],
    ])
}

pub fn build_contract_duration_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.contract_duration.1_year"),
                "rent:duration:1",
            ),
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.contract_duration.2_years"),
                "rent:duration:2",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.contract_duration.3_years"),
                "rent:duration:3",
            ),
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.contract_duration.other"),
                "rent:duration:other",
            ),
        ],
    ])
}

#[allow(dead_code)]
pub fn build_due_day_keyboard() -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    for row_start in (1..=31).step_by(7) {
        let row: Vec<InlineKeyboardButton> = (row_start..=(row_start + 6).min(31))
            .map(|day| {
                InlineKeyboardButton::callback(day.to_string(), format!("rent:due_day:{}", day))
            })
            .collect();
        rows.push(row);
    }
    InlineKeyboardMarkup::new(rows)
}

#[allow(dead_code)]
pub fn build_reminder_timing_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step7_reminder_timing.same_day"),
                "rent:timing:same_day",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_day_before",
                ),
                "rent:timing:1_day_before",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.3_days_before",
                ),
                "rent:timing:3_days_before",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_week_before",
                ),
                "rent:timing:1_week_before",
            ),
        ],
    ])
}

pub fn build_pre_reminder_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step9_pre_reminder.1_day_before"),
                "rent:timing:1_day_before",
            ),
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step9_pre_reminder.3_days_before"),
                "rent:timing:3_days_before",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step9_pre_reminder.1_week_before"),
                "rent:timing:1_week_before",
            ),
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step9_pre_reminder.no_extra"),
                "rent:timing:same_day",
            ),
        ],
    ])
}

pub fn build_confirm_keyboard(language: &str, callback_prefix: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        t(language, "common.confirm_button"),
        format!("{}:confirm", callback_prefix),
    )]])
}

pub fn build_agreements_list_keyboard(
    language: &str,
    agreements: &[Agreement],
) -> InlineKeyboardMarkup {
    let view_text = t(language, "agreement.list.view_button");
    let delete_text = t(language, "agreement.list.delete_button");

    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    for agreement in agreements {
        let icon = if agreement.agreement_type == "rent" {
            "🏠"
        } else {
            "📝"
        };
        let title_row = vec![InlineKeyboardButton::callback(
            format!("{} {}", icon, &agreement.title),
            format!("agr:view:{}", agreement.id),
        )];
        rows.push(title_row);

        let action_row = vec![
            InlineKeyboardButton::callback(view_text.clone(), format!("agr:view:{}", agreement.id)),
            InlineKeyboardButton::callback(
                delete_text.clone(),
                format!("agr:delete:{}", agreement.id),
            ),
        ];
        rows.push(action_row);
    }

    InlineKeyboardMarkup::new(rows)
}

pub fn build_agreement_detail_keyboard(
    language: &str,
    agreement_id: i32,
    agreement_type: &str,
) -> InlineKeyboardMarkup {
    let edit_text = t(language, "agreement.view.edit_button");
    let delete_text = t(language, "agreement.view.delete_button");
    let back_text = t(language, "agreement.view.back_button");

    let mut rows = vec![vec![
        InlineKeyboardButton::callback(edit_text, format!("agr:edit:{}", agreement_id)),
        InlineKeyboardButton::callback(delete_text, format!("agr:delete:{}", agreement_id)),
    ]];

    rows.push(vec![InlineKeyboardButton::callback(
        back_text,
        "agr:back:list",
    )]);

    let _ = agreement_type;

    InlineKeyboardMarkup::new(rows)
}

pub fn build_delete_confirm_keyboard(language: &str, agreement_id: i32) -> InlineKeyboardMarkup {
    let confirm_text = t(language, "agreement.delete.confirm_button");
    let cancel_text = t(language, "agreement.delete.cancel_button");

    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            confirm_text,
            format!("agr:delete:confirm:{}", agreement_id),
        ),
        InlineKeyboardButton::callback(cancel_text, "agr:back:list"),
    ]])
}

pub fn build_edit_menu_keyboard(
    language: &str,
    agreement_id: i32,
    agreement_type: &str,
) -> InlineKeyboardMarkup {
    let mut rows = vec![];

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "agreement.edit.title_button"),
        format!("agr:edit:{}:title", agreement_id),
    )]);

    if agreement_type == "rent" {
        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.amount_button"),
            format!("agr:edit:{}:amount", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.due_day_button"),
            format!("agr:edit:{}:due_day", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.monthly_reminder_button"),
            format!("agr:edit:{}:monthly", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.reminder_timing_button"),
            format!("agr:edit:{}:timing", agreement_id),
        )]);

        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.yearly_increase_button"),
            format!("agr:edit:{}:yearly", agreement_id),
        )]);
    } else {
        rows.push(vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.description_button"),
            format!("agr:edit:{}:description", agreement_id),
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        t(language, "agreement.edit.back_button"),
        format!("agr:view:{}", agreement_id),
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn build_edit_timing_keyboard(language: &str, agreement_id: i32) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.timing_before"),
            format!("agr:edit:{}:timing_before", agreement_id),
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.timing_on_day"),
            format!("agr:edit:{}:timing_on_day", agreement_id),
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.edit.back_button"),
            format!("agr:edit:{}", agreement_id),
        )],
    ])
}

pub fn build_edit_due_day_keyboard(agreement_id: i32) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    for row_start in (1..=28).step_by(7) {
        let row: Vec<InlineKeyboardButton> = (row_start..row_start + 7)
            .filter(|&d| d <= 28)
            .map(|d| {
                InlineKeyboardButton::callback(
                    d.to_string(),
                    format!("agr:edit:{}:due_day_{}", agreement_id, d),
                )
            })
            .collect();
        rows.push(row);
    }

    InlineKeyboardMarkup::new(rows)
}

pub fn build_custom_currency_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🇹🇷 TRY", "custom:currency:TRY"),
            InlineKeyboardButton::callback("🇪🇺 EUR", "custom:currency:EUR"),
        ],
        vec![
            InlineKeyboardButton::callback("🇺🇸 USD", "custom:currency:USD"),
            InlineKeyboardButton::callback("🇬🇧 GBP", "custom:currency:GBP"),
        ],
    ])
}

pub fn build_custom_timing_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                t(language, "agreement.rent.step7_reminder_timing.same_day"),
                "custom:timing:same_day",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_day_before",
                ),
                "custom:timing:1_day_before",
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.3_days_before",
                ),
                "custom:timing:3_days_before",
            ),
            InlineKeyboardButton::callback(
                t(
                    language,
                    "agreement.rent.step7_reminder_timing.1_week_before",
                ),
                "custom:timing:1_week_before",
            ),
        ],
    ])
}

pub fn build_reminder_list_keyboard(language: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.custom.add_reminder.add_another"),
            "custom:add_another",
        )],
        vec![InlineKeyboardButton::callback(
            t(language, "agreement.custom.add_reminder.finish"),
            "custom:finish",
        )],
    ])
}

pub fn build_snooze_options_keyboard(reminder_id: i32, lang: &str) -> InlineKeyboardMarkup {
    let row1 = vec![
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_1h"),
            format!("rem:snooze_1h:{}", reminder_id),
        ),
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_3h"),
            format!("rem:snooze_3h:{}", reminder_id),
        ),
    ];

    let row2 = vec![
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_1d"),
            format!("rem:snooze_1d:{}", reminder_id),
        ),
        InlineKeyboardButton::callback(
            t(lang, "agreement.reminder.snooze_3d"),
            format!("rem:snooze_3d:{}", reminder_id),
        ),
    ];

    InlineKeyboardMarkup::new(vec![row1, row2])
}
