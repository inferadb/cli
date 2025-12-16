# InferaDB CLI Development Overview

An ideal InferaDB CLI should feel familiar to people who use `kubectl`, `git`, `aws`, and `fga`—composable subcommands, great `--help`, clear resources, and safe defaults.

## Core Design Principles

- Nouns and verbs: `inferadb vaults list`, `inferadb evaluate`.
- Idempotent commands: `apply` and `sync` should be safe to re-run.
- First-class environments: `--env staging`, `--endpoint`, profiles in config file.
- Human-friendly by default, machine-friendly via `-o json|yaml`.

## CLI Profiles

Users can configure multiple profiles to switch between different environments. Profiles are stored in the user's home directory in a file called `.inferadb/profiles.yaml`.

### Create a CLI profile

```bash
# Production operations configuration:
inferadb profiles create prod --url https://api.inferadb.com

# Local development configuration:
inferadb profiles create dev --url http://localhost --default
```

### List CLI profiles

```bash
inferadb profiles list
```

### Delete a CLI profile

```bash
inferadb profiles delete prod
```

### Update a CLI profile

```bash
inferadb profiles update prod --url https://api.inferadb.com
```

### Rename a CLI profile

```bash
inferadb profiles rename prod new-prod
```

### Set a default CLI profile

```bash
inferadb profiles default prod
```

## Service Status

```bash
inferadb status --profile example
```

Displays the status of the configured InferaDB service and its components by querying the health endpoints. Also returns the current CLI profile authentication status with the service.

## Authentication

The CLI will associate and store authentication details with individual profiles. In this way, a user can be logged into a production cluster with one profile, a local development environment in another, and so on.

### Register

```bash
inferadb register hello@evansims.com "Evan Sims" --profile example
```

### Login

This should use the OAuth flow outlined in /control/docs/authentication.md.

```bash
inferadb login --profile example
```

### Logout

```bash
inferadb logout --profile example
```

## Account Management

Manage account details using Control endpoints.

### Get Account Details

```bash
inferadb account
```

### Update Account Details

```bash
inferadb account update
```

### Delete Account

```bash
inferadb account delete
```

### Email Management

#### List Emails

```bash
inferadb emails list
```

#### Add an Email

```bash
inferadb emails add {email}
```

### Session Management

Manage sessions using Control endpoints.

#### List sessions

```bash
inferadb sessions list
```

#### Revoke a session

```bash
inferadb sessions revoke {session-snowflake-id}
```

#### Revoke all other sessions

```bash
inferadb sessions revoke-others
```

## Organization Mnaagement

List and inspect organizations using Control endpoints.

### List your organizations

```bash
inferadb orgs
```

### Create an organization

```bash
inferadb orgs create "Example Organization"
```

This command should return the Snowflake ID of the new organization.

This command should list the organizations a user has access to, with the Snowflake IDs and names.

### Get organization details

```bash
inferadb orgs {org-snowflake-id} describe
```

### Join an organization

Joins an organization by accepting an invitation.

```bash
inferadb orgs {org-snowflake-id} join {invite-snowflake-id}
```

### Leave an organization

Removes your membership with an organization, unless you are the sole owner of it.

This is a destructive operation, so it should require confirmation from the user before proceeding. For non-interactive scripts, it should accept a `--yes` flag to bypass confirmation.

```bash
inferadb orgs {org-snowflake-id} leave
```

### Suspend an Organization

```bash
inferadb orgs {org-snowflake-id} suspend
```

### Resume an Organization

```bash
inferadb orgs {org-snowflake-id} resume
```

### Manage organization members

#### List Members

```bash
inferadb orgs {org-snowflake-id} members
```

#### Remove Member

```bash
inferadb orgs {org-snowflake-id} members remove {member-snowflake-id}
```

### Manage organization teams

#### List Teams

```bash
inferadb orgs {org-snowflake-id} teams
```

#### Create a Team

```bash
inferadb orgs {org-snowflake-id} teams create {team-name}
```

This will return the Snowflake ID of the new team.

#### Delete a Team

```bash
inferadb orgs {org-snowflake-id} teams delete {team-snowflake-id}
```

### Manage user grants

#### Create a user grant

```bash
inferadb orgs {org-snowflake-id} user-grants create {user-snowflake-id} {role}
```

#### List user grants

```bash
inferadb orgs {org-snowflake-id} user-grants
```

#### Update a user grant

```bash
inferadb orgs {org-snowflake-id} user-grants update {user-snowflake-id} {role}
```

#### Delete a user grant

```bash
inferadb orgs {org-snowflake-id} user-grants delete {user-snowflake-id}
```

### Manage team permissions

#### List team permissions

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} permissions
```

#### Grant a team permission

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} permissions grant {permission}
```

#### Revoke a team permission

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} permissions revoke {permission}
```

### Manage team grants

#### Create a team grant

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} grants create {role}
```

#### List team grants

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} grants
```

#### Update a team grant

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} grants update {role}
```

#### Delete a team grant

```bash
inferadb orgs {org-snowflake-id} teams {team-snowflake-id} grants delete
```

### Manage organization invites

Users can be added to an organization by inviting them to join. Invites are sent through email by the server.

#### List all pending invitations

```bash
inferadb orgs {org-snowflake-id} invitations
```

#### Invite a user to join an organization

```bash
inferadb orgs {org-snowflake-id} invitations create {email}
```

#### Revoke (delete) an invitation

```bash
inferadb orgs {org-snowflake-id} invitations delete {invite-snowflake-id}
```

### Vault Management

List and inspect organization vaults using Control endpoints.

#### List Your Vaults

```bash
inferadb orgs {org-snowflake-id} vaults
```

This command will list a table with all the Snowflake IDs, names and descriptions of the vaults the user has access to.

#### Create a Vault

```bash
inferadb orgs {org-snowflake-id} vaults create "{vault-name}" --description "{vault-description}"
```

This command will return a Snowflake ID for the new vault.

#### Update a Vault

```bash
inferadb orgs {org-snowflake-id} vaults update {vault-snowflake-id}
```

#### Delete a Vault

```bash
inferadb orgs {org-snowflake-id} vaults delete {vault-snowflake-id}
```

#### Get Vault Details

```bash
inferadb orgs {org-snowflake-id} vaults describe {vault-snowflake-id}
```

### Clients Management

List and inspect clients and their certificates using Control endpoints.

#### List Clients

```bash
inferadb orgs {org-snowflake-id} clients
```

#### Create a Client

```bash
inferadb orgs {org-snowflake-id} clients create
```

#### Get Client Details

```bash
inferadb orgs {org-snowflake-id} clients describe {client-snowflake-id}
```

#### Delete a Client

```bash
inferadb orgs {org-snowflake-id} clients delete {client-snowflake-id}
```

#### Update a Client

```bash
inferadb orgs {org-snowflake-id} clients update {client-snowflake-id}
```

#### Manage Client Certificates

##### List Client Certificates

```bash
inferadb orgs {org-snowflake-id} clients certificates
```

##### Create a Client Certificate

```bash
inferadb orgs {org-snowflake-id} clients certificates create
```

##### Get Client Certificate Details

```bash
inferadb orgs {org-snowflake-id} clients certificates describe {cert-snowflake-id}
```

##### Revoke a Client Certificate

```bash
inferadb orgs {org-snowflake-id} clients certificates revoke {cert-snowflake-id}
```

### Schema Management

Manage organization vault schemas using Control endpoints.

#### List Schemas

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas
```

#### Create a Schema

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas create
```

#### Get a Schema

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas describe {schema-snowflake-id}
```

#### Delete a Schema

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas delete {schema-snowflake-id}
```

#### Get the current active schema for a vault

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas current
```

#### Get the diff between two schema versions for a vault

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas diff {schema-snowflake-id} {schema-snowflake-id}
```

#### Rollback a Vault Schema

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas rollback {schema-snowflake-id}
```

#### Activate a Vault Schema

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} schemas activate {schema-snowflake-id}
```

### Token Management

Manage organizationvault tokens using Control endpoints.

#### List Vault Tokens

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} tokens
```

#### Generate a Vault Token

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} tokens generate
```

#### Revoke Vault Tokens

```bash
inferadb orgs {org-snowflake-id} vaults {vault-snowflake-id} tokens revoke {token-snowflake-id}
```

### Logs

#### List Logs

```bash
inferadb orgs {org-snowflake-id} logs
```

## Authorization Queries

### Evaluate an authorization query

```bash
inferadb evaluate {subject} {resource} {permission} --vault {vault-snowflake-id}
```

### Expand a relationship

```bash
inferadb expand {subject} {resource} --vault {vault-snowflake-id}
```

### List authorized resources

```bash
inferadb list {subject} {resource} --vault {vault-snowflake-id}
```

### List authorized subjects

```bash
inferadb list {subject} {resource} --vault {vault-snowflake-id}
```

### List relationships

```bash
inferadb list {subject} {resource} --vault {vault-snowflake-id}
```

### List subjects

```bash
inferadb list {subject} {resource} --vault {vault-snowflake-id}
```

### Watch for real-time relationship changes

```bash
inferadb watch {subject} {resource} --vault {vault-snowflake-id}
```

### Simulate an authorization query

```bash
inferadb simulate {subject} {resource} {permission} --vault {vault-snowflake-id}
```

### Write a relationship

```bash
inferadb write {subject} {resource} {permission} --vault {vault-snowflake-id}
```

### Delete a relationship

```bash
inferadb delete {subject} {resource} {permission} --vault {vault-snowflake-id}
```

## Planned Features (requires new API endpoints)

- `inferadb export`
- `inferadb import`
- Service Operator commands, available only to service administrators:
  - `inferadb sop accounts list` to list serice operators
  - `inferadb sop accounts add {account-snowflake-id}` to add a service operator
  - `inferadb sop accounts remove {account-snowflake-id}` to remove a service operator
  - `inferadb bench --concurrency 32 --duration 60s --scenario examples/scenarios/org-rbac.yaml` to run a performance benchmark test
  - `inferadb slos` to get the SLOs
    - `inferadb slos burn-rate --window 6h` to get the burn rate of the service
- Local development commands:
  - `inferadb dev up` to start a local development cluster
  - `inferadb dev down` to tear down a local development cluster
  - `inferadb dev logs --follow` to follow the logs of the local development cluster
  - `inferadb dev status` to get the status of the local development cluster
  - `inferadb dev import example.json` to seed the local development cluster with data
  - `inferadb dev export example.json` to export the data from the local development cluster
  - `inferadb dev dashboard` to open the dashboard of the local development cluster

## Sources

[1] Command line tool (kubectl) - Kubernetes <https://kubernetes.io/docs/reference/kubectl/>
[2] 15 Developer Experience Best Practices for Engineering Teams <https://jellyfish.co/blog/developer-experience-best-practices/>
[3] Enrich Auth0 Access Tokens with Auth0 FGA Data <https://auth0.com/blog/enrich-auth0-access-tokens-with-auth0-fga-data/>
[4] Authenticating using IAM user credentials for the AWS CLI <https://docs.aws.amazon.com/cli/latest/userguide/cli-authentication-user.html>
[5] 12 CLI Tools That Are Redefining Developer Workflows - Qodo <https://www.qodo.ai/blog/best-cli-tools/>
[6] kind - Kubernetes <https://kind.sigs.k8s.io>
[7] dockersamples/wordsmith: Sample project with Docker ... - GitHub <https://github.com/dockersamples/wordsmith>
[8] Demoing deployment of Docker containers into Kubernetes ... - GitHub <https://github.com/HoussemDellai/ProductsStoreOnKubernetes>
[9] Building Docker images in Kubernetes - Snyk <https://snyk.io/blog/building-docker-images-kubernetes/>
[10] Projects - Awesome Kubernetes - Ramit Surana <https://ramitsurana.github.io/awesome-kubernetes/projects/projects/>
[11] Supercharge Your Authorization System with FGA | Auth0 <https://auth0.com/blog/supercharge-your-authorization-system-with-openfga/>
[12] Open source CLI and template for local Kubernetes microservice ... <https://www.reddit.com/r/devops/comments/1o7hrbt/open_source_cli_and_template_for_local_kubernetes/>
[13] 15 Best Developer Experience Tools to Look for in 2025 | Milestone <https://mstone.ai/blog/15-best-developer-experience-tools-2025/>
[14] Full Kubernetes tutorial on Docker, KinD, kubectl, Helm ... - YouTube <https://www.youtube.com/watch?v=SeQevrW176A>
[15] Streamlining API Security with AWS Lambda Authorizers and Auth0 ... <https://auth0.com/blog/api-security-with-aws-lambda-authorizers-and-okta-fga/>
[16] 8 Developer Tools You Should Try in 2024 - DEV Community <https://dev.to/studio1hq/8-developer-tools-you-should-try-in-2024-b8c>
[17] 13 Kubernetes CLI Tools You Should Know - overcast blog <https://overcast.blog/13-kubernetes-cli-tools-you-should-know-439270d27257>
[18] 10 CLI Tools That Made the Biggest Impact On Transforming My ... <https://www.reddit.com/r/commandline/comments/1epjppl/10_cli_tools_that_made_the_biggest_impact_on/>
[19] A Step-by-Step Guide to Securing Amazon Bedrock Agents with Auth0 <https://auth0.com/blog/securing-amazon-bedrock-agents-with-auth0-genai-guide/>
[20] CLI Tools every Developer should know - CodeParrot AI <https://codeparrot.ai/blogs/cli-tools-every-developer-should-know>
