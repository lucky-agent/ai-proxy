# request-editor-tabs Specification

## Purpose
TBD - created by archiving change new-request-tabs. Update Purpose after archive.
## Requirements
### Requirement: Tab opening from tree node

The system SHALL open a request editor tab when the user clicks a request node in the API collection tree. If a tab already linked to that tree node exists, the system SHALL activate that tab instead of creating a new one.

#### Scenario: Click request node opens new tab
- **WHEN** user clicks a request node "GET /api/users" in the collection tree, and no tab linked to that node exists
- **THEN** a new tab labeled "GET /api/users" opens, becomes active, and loads the node's method, URL, params, headers, cookies, body

#### Scenario: Click same node activates existing tab
- **WHEN** user clicks the same request node again, and a tab linked to that node already exists
- **THEN** the existing tab becomes active without creating a new one

### Requirement: Independent tab state

Each tab SHALL maintain its own independent editing state (method, URL, params, headers, cookies, body, bodyType). Modifications in one tab MUST NOT affect any other tab.

#### Scenario: Edit in tab A does not affect tab B
- **WHEN** user opens tab A linked to "GET /api/users" and tab B linked to "POST /api/orders"
- **AND** user modifies the URL in tab A
- **THEN** tab B's URL remains unchanged when user switches back to tab B

### Requirement: Real-time sync for linked tabs

When a tab is linked to a tree node (`linkedNodeId` is non-null), any edit to the tab's content SHALL be debounced and automatically synced back to the corresponding tree node via the existing `useCollections.updateRequest` mechanism.

#### Scenario: Edit linked tab syncs to tree
- **WHEN** user edits the URL in a tab linked to "GET /api/users"
- **AND** the debounce interval (300ms) elapses
- **THEN** the tree node's `url` field is updated via `updateRequest`
- **AND** the change is eventually persisted to persistent storage

#### Scenario: Temporary tab does not sync
- **WHEN** user edits the URL in a temporary tab (`linkedNodeId: null`)
- **THEN** no tree node update is triggered

### Requirement: Temporary tab creation

The system SHALL provide a [+] button in the request tab bar that creates a new unnamed temporary tab. The tab SHALL have default empty state and no association with any tree node.

#### Scenario: Create temporary tab via [+] button
- **WHEN** user clicks the [+] button in the request tab bar
- **THEN** a new tab with default name (i18n equivalent of "Untitled Request") is created and activated
- **AND** the tab has method=GET, empty URL, empty params/headers/cookies/body

### Requirement: Tab closing

The system SHALL allow closing any tab via a close button (✕) on the tab. No confirmation dialog SHALL be shown. The tab's unsaved state SHALL be discarded immediately. If the closed tab is currently active, the system SHALL activate the nearest adjacent tab. When the last tab is closed, the system SHALL display an empty state placeholder view.

#### Scenario: Close tab discards state
- **WHEN** user closes a temporary tab that has been edited
- **THEN** the tab closes immediately without confirmation
- **AND** the edited data is not persisted

#### Scenario: Close active tab activates adjacent
- **WHEN** 3 tabs are open (A, B, C) and tab B is active
- **AND** user closes tab B
- **THEN** tab C becomes active (or tab A if C was to the left)

#### Scenario: Close last tab shows empty state
- **WHEN** only 1 tab is open
- **AND** user closes that tab
- **THEN** the right panel displays an empty state placeholder view

### Requirement: Independent response per tab

Each tab SHALL maintain its own send request state (sending flag, error message) and response result (`responseEntryId`). When the user sends a request in one tab and switches to another, the previous tab's response SHALL still be visible when switching back.

#### Scenario: Response preserved across tab switch
- **WHEN** user sends a request in tab A and receives a response
- **AND** user switches to tab B
- **AND** user switches back to tab A
- **THEN** tab A's response (DetailPanel) is still displayed with the previous result

#### Scenario: Send in one tab only affects that tab
- **WHEN** user sends a request in tab A
- **THEN** tab B's response area is unaffected
- **AND** tab B's `sending` flag remains false

