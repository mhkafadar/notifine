UPDATE reminders SET status = 'pending' WHERE status IN ('sending', 'failed');
ALTER TABLE reminders DROP CONSTRAINT reminders_status_check;
ALTER TABLE reminders ADD CONSTRAINT reminders_status_check
  CHECK (status IN ('pending', 'sent', 'done', 'overdue', 'archived'));
