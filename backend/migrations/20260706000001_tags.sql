-- Tags for monitors
ALTER TABLE monitors ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';