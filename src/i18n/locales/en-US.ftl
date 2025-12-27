# English (US) translations for InferaDB CLI
# This file uses Project Fluent syntax: https://projectfluent.org/

## ============================================================================
## Error Messages
## ============================================================================

error-auth-required = Authentication required. Run 'inferadb login' first.
error-profile-not-found = Profile '{ $name }' not found.
error-config-not-found = Configuration file not found: { $path }
error-config-parse = Failed to parse configuration: { $details }
error-connection-failed = Connection failed: { $details }
error-api-error = API error: { $message }
error-invalid-argument = Invalid argument: { $details }
error-vault-required = Vault ID required. Set with --vault or in profile.
error-org-required = Organization ID required. Set with --org or in profile.
error-permission-denied = Permission denied.
error-not-found = Resource not found: { $resource }
error-conflict = Conflict: { $details }
error-timeout = Request timed out.
error-unknown = An unknown error occurred.

## ============================================================================
## CLI Help Text - Main
## ============================================================================

cli-about = InferaDB CLI - Debug authorization decisions, test policies, and manage tenants from your terminal.
cli-profile-help = Profile to use (or use @profile prefix)
cli-org-help = Organization ID (overrides profile)
cli-vault-help = Vault ID (overrides profile)
cli-output-help = Output format: table, json, yaml, jsonl
cli-color-help = Color output: auto, always, never
cli-quiet-help = Suppress non-essential output
cli-yes-help = Skip confirmation prompts
cli-debug-help = Enable debug logging

## ============================================================================
## CLI Help Text - Commands
## ============================================================================

# Auth commands
cmd-login-about = Authenticate with InferaDB
cmd-login-provider-help = OAuth provider to use
cmd-logout-about = Clear authentication credentials
cmd-register-about = Register a new account
cmd-init-about = Initialize a new project

# Identity commands
cmd-whoami-about = Show current authentication status
cmd-status-about = Show connection and authentication status
cmd-ping-about = Check connectivity to InferaDB API
cmd-doctor-about = Diagnose common issues

# Check commands
cmd-check-about = Check if a subject has permission on a resource
cmd-check-subject-help = Subject to check (e.g., user:alice)
cmd-check-permission-help = Permission to check (e.g., can_view)
cmd-check-resource-help = Resource to check (e.g., doc:readme)
cmd-check-context-help = Additional context as JSON
cmd-simulate-about = Simulate a check with debug output
cmd-expand-about = Expand a userset to its members

# Profile commands
cmd-profiles-about = Manage CLI profiles
cmd-profiles-list-about = List all profiles
cmd-profiles-show-about = Show profile details
cmd-profiles-add-about = Add a new profile
cmd-profiles-remove-about = Remove a profile
cmd-profiles-set-default-about = Set the default profile

# Relationship commands
cmd-relationships-about = Manage relationships
cmd-relationships-list-about = List relationships
cmd-relationships-add-about = Add a relationship
cmd-relationships-remove-about = Remove a relationship

# Schema commands
cmd-schemas-about = Manage authorization schemas
cmd-schemas-list-about = List schemas
cmd-schemas-show-about = Show schema details
cmd-schemas-apply-about = Apply a schema from file
cmd-schemas-validate-about = Validate a schema file

# Dev commands
cmd-dev-about = Development and debugging tools
cmd-dev-doctor-about = Run diagnostics
cmd-dev-shell-about = Open interactive shell
cmd-dev-completions-about = Generate shell completions

# Other commands
cmd-cheatsheet-about = Show common commands and examples

## ============================================================================
## Command Output Messages
## ============================================================================

# Authentication
msg-login-success = Login successful!
msg-login-failed = Login failed: { $reason }
msg-logout-success = Logged out from profile '{ $profile }'.
msg-not-authenticated = Not authenticated.
msg-authenticated-as = Authenticated as { $identity }
msg-auth-status-yes = Authenticated: yes
msg-auth-status-no = Authenticated: no

# Init wizard
msg-init-welcome = Welcome to InferaDB CLI!
msg-init-already-configured = You already have profiles configured.
msg-init-create-new-profile = Do you want to create a new profile?
msg-init-profile-created = Profile '{ $name }' created and set as default!
msg-init-all-set = You're all set! Try:
msg-init-try-whoami = inferadb whoami
msg-init-try-check = inferadb check user:alice can_view document:readme
msg-init-enter-ids = Enter your organization and vault IDs.
msg-init-find-in-dashboard = You can find these in the InferaDB dashboard.

# Login/logout
msg-logging-in = Logging in as profile '{ $profile }'...
msg-logging-out = Log out from profile '{ $profile }'?
msg-not-logged-in = Profile '{ $profile }' is not logged in.
msg-cancelled = Cancelled.

# Registration
msg-registration-not-implemented = Registration not yet implemented.
msg-registration-email-name = Email: { $email }, Name: { $name }
msg-email-name-required = Email and name are required

# Prompts
prompt-profile-name = Profile name (default: 'default'):
prompt-api-url = API URL (default: https://api.inferadb.com):
prompt-org-id = Organization ID:
prompt-vault-id = Vault ID:
prompt-email = Email:
prompt-name = Name:

# Profiles
msg-profile-created = Profile '{ $name }' created.
msg-profile-updated = Profile '{ $name }' updated.
msg-profile-removed = Profile '{ $name }' removed.
msg-profile-set-default = Default profile set to '{ $name }'.
msg-no-profiles = No profiles configured.
msg-profiles-header = Profiles:
msg-default-indicator = (default)

# Check results
msg-check-allowed = { $subject } { $permission } { $resource } → allowed
msg-check-denied = { $subject } { $permission } { $resource } → denied
msg-check-result-allowed = allowed
msg-check-result-denied = denied

# Relationships
msg-relationship-added = Relationship added: { $resource } { $relation } { $subject }
msg-relationship-removed = Relationship removed: { $resource } { $relation } { $subject }
msg-no-relationships = No relationships found.

# Schemas
msg-schema-applied = Schema applied successfully.
msg-schema-valid = Schema is valid.
msg-schema-invalid = Schema validation failed: { $errors }
msg-no-schemas = No schemas found.

# Connection
msg-ping-success = Connected to { $endpoint } ({ $latency }ms)
msg-ping-failed = Connection failed: { $reason }
msg-connection-ok = Connection: OK
msg-connection-failed = Connection: FAILED

# Doctor/diagnostics
msg-doctor-header = InferaDB CLI Diagnostics
msg-doctor-checking = Checking { $item }...
msg-doctor-ok = OK
msg-doctor-warning = Warning
msg-doctor-error = Error
msg-doctor-all-ok = All checks passed!
msg-doctor-issues-found = { $count } issue(s) found.

## ============================================================================
## Table Headers
## ============================================================================

table-name = NAME
table-value = VALUE
table-status = STATUS
table-profile = PROFILE
table-endpoint = ENDPOINT
table-org = ORG
table-vault = VAULT
table-default = DEFAULT
table-subject = SUBJECT
table-relation = RELATION
table-resource = RESOURCE
table-permission = PERMISSION
table-result = RESULT
table-schema = SCHEMA
table-version = VERSION
table-created = CREATED
table-updated = UPDATED

## ============================================================================
## Prompts and Confirmations
## ============================================================================

prompt-confirm-delete = Are you sure you want to delete '{ $name }'?
prompt-confirm-logout = Are you sure you want to log out?
prompt-yes-no = [y/N]
prompt-enter-value = Enter { $field }:
prompt-select-profile = Select a profile:

## ============================================================================
## Progress and Status
## ============================================================================

progress-connecting = Connecting...
progress-authenticating = Authenticating...
progress-loading = Loading...
progress-saving = Saving...
progress-checking = Checking authorization...

## ============================================================================
## Cheatsheet
## ============================================================================

cheatsheet-title = InferaDB CLI Cheatsheet
cheatsheet-auth-section = Authentication
cheatsheet-check-section = Authorization Checks
cheatsheet-relationships-section = Relationships
cheatsheet-profiles-section = Profiles
