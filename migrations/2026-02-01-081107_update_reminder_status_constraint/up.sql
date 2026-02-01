ALTER TABLE reminders DROP CONSTRAINT reminders_status_check;
ALTER TABLE reminders ADD CONSTRAINT reminders_status_check
  CHECK (status IN ('pending', 'sending', 'sent', 'done', 'failed', 'overdue'));
