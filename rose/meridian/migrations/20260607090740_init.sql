-- Create "authors" table
CREATE TABLE `authors` (`id` integer NULL, `name` text NOT NULL, `email` text NOT NULL, `created_at` timestamp NOT NULL DEFAULT (CURRENT_TIMESTAMP), PRIMARY KEY (`id`));
-- Create index "authors_email_idx" to table: "authors"
CREATE UNIQUE INDEX `authors_email_idx` ON `authors` (`email`);
-- Create "works" table
CREATE TABLE `works` (`id` integer NULL, `title` text NOT NULL, `author_id` integer NOT NULL, `year` integer NULL, `created_at` timestamp NOT NULL DEFAULT (CURRENT_TIMESTAMP), PRIMARY KEY (`id`), CONSTRAINT `0` FOREIGN KEY (`author_id`) REFERENCES `authors` (`id`) ON UPDATE NO ACTION ON DELETE NO ACTION);
-- Create index "works_author_id_idx" to table: "works"
CREATE INDEX `works_author_id_idx` ON `works` (`author_id`);
