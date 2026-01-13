ALTER TABLE reminders DROP CONSTRAINT reminders_reminder_type_check;
ALTER TABLE reminders ADD CONSTRAINT reminders_reminder_type_check
    CHECK (reminder_type IN ('pre_notify', 'due_day', 'yearly_increase', 'ten_year_notice', 'five_year_notice'));

ALTER TABLE agreements ADD COLUMN has_five_year_reminder BOOLEAN NOT NULL DEFAULT true;
