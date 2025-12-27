CREATE TABLE agreements (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES agreement_users(id) ON DELETE CASCADE,
    agreement_type VARCHAR(20) NOT NULL CHECK (agreement_type IN ('rent', 'custom')),
    title VARCHAR(50) NOT NULL,
    user_role VARCHAR(20) CHECK (user_role IN ('tenant', 'landlord')),
    start_date DATE,
    currency VARCHAR(3) NOT NULL DEFAULT 'TRY' CHECK (currency IN ('TRY', 'EUR', 'USD', 'GBP')),
    rent_amount DECIMAL(15, 2),
    due_day INTEGER CHECK (due_day >= 1 AND due_day <= 31),
    has_monthly_reminder BOOLEAN NOT NULL DEFAULT false,
    reminder_timing VARCHAR(20) CHECK (reminder_timing IN ('same_day', '1_day_before', '3_days_before', '1_week_before', 'custom')),
    reminder_days_before INTEGER,
    has_yearly_increase_reminder BOOLEAN NOT NULL DEFAULT false,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_user_title UNIQUE (user_id, title)
);

CREATE INDEX idx_agreements_user_id ON agreements(user_id);
CREATE INDEX idx_agreements_type ON agreements(agreement_type);
