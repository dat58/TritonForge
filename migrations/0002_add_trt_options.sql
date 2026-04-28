-- Add trt_options column to conversion_jobs table
ALTER TABLE conversion_jobs ADD COLUMN trt_options TEXT NOT NULL DEFAULT '{}';
