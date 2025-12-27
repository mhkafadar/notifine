CREATE TABLE reminders (
    id SERIAL PRIMARY KEY,
    agreement_id INTEGER NOT NULL REFERENCES agreements(id) ON DELETE CASCADE,
    reminder_type VARCHAR(20) NOT NULL CHECK (reminder_type IN ('pre_notify', 'due_day', 'yearly_increase')),
    title VARCHAR(100) NOT NULL,
    amount DECIMAL(15, 2),
    due_date DATE NOT NULL,
    reminder_date DATE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'done', 'overdue', 'archived')),
    snooze_count INTEGER NOT NULL DEFAULT 0 CHECK (snooze_count >= 0 AND snooze_count <= 3),
    snoozed_until TIMESTAMPTZ,
    sent_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reminders_agreement_id ON reminders(agreement_id);
CREATE INDEX idx_reminders_status ON reminders(status);
CREATE INDEX idx_reminders_due_date ON reminders(due_date);
CREATE INDEX idx_reminders_reminder_date ON reminders(reminder_date);
CREATE INDEX idx_reminders_pending ON reminders(status, reminder_date) WHERE status = 'pending';
