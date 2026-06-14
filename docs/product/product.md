# Burnly Product Document

## Product Summary

Burnly is a desktop app that helps developers understand how they use AI coding tools.

It brings usage from tools such as Claude Code, Codex, OpenCode, and others into one clear, consistent experience. Instead of checking separate commands or reports, developers can see their activity, token consumption, estimated cost, and usage patterns in one place.

Burnly is local-first and private by default. A future optional online account will let users sync selected usage data, view a web dashboard, and participate in community features.

## Vision

Make AI coding usage understandable, intentional, and easy to track.

Burnly aims to become the activity tracker for AI-assisted software development: a place where developers can understand their habits across tools, projects, and time.

## Product Principles

### Private by default

Usage stays on the user's device unless they explicitly choose to sync it. Burnly should never collect prompts, responses, source code, or other sensitive content for its core experience.

### One clear view

Different coding tools report usage differently. Burnly should present a consistent experience without requiring users to understand each tool's reporting model.

### Useful at a glance

The most important information should be available quickly from the system tray, without requiring users to open the full app.

### Honest metrics

Token usage and estimated cost do not always mean the same thing across tools and plans. Burnly should clearly distinguish measured usage from estimates and avoid implying false precision.

### Insight over volume

Burnly should help users understand and manage their usage, not encourage unnecessary token consumption.

## Target Users

### Primary audience

- Developers who regularly use more than one AI coding tool
- Developers who want to monitor token usage or estimated cost
- Developers who want a history of their AI-assisted coding activity
- Privacy-conscious users who prefer local tools

### Future audience

- Teams that want visibility into AI tool adoption and spending
- Engineering managers evaluating usage patterns
- Developers who want to share selected activity publicly
- Organizations managing usage across multiple providers

## Core User Problems

- Usage information is spread across different tools and commands.
- Each tool presents tokens, costs, sessions, and models differently.
- Developers cannot easily see their total usage across tools.
- It is difficult to understand usage trends over days, weeks, or months.
- Developers may exceed a budget before noticing a change in usage.
- Existing reports are not always convenient to check during the workday.
- Users lack a private, long-term activity history they control.

## Product Experience

### Overview

The overview gives users an immediate picture of their AI coding activity for a selected period.

It should answer:

- How much have I used today?
- How does this compare with previous periods?
- Which tools and models account for the most usage?
- What is my estimated cost?
- Am I approaching a budget limit?

### Activity Calendar

The activity calendar is a contribution-style calendar heatmap. Each day is represented by a cell, and its intensity reflects the selected activity metric.

Users can view activity by:

- Tokens
- Estimated cost
- Sessions
- Active days

Selecting a day reveals its usage breakdown. The calendar helps users recognize streaks, unusually heavy days, and long-term patterns.

### Usage Breakdown

Users can explore usage by:

- Coding tool
- Model
- Project, when available
- Session
- Time period

The experience should make comparison easy while explaining when a metric is unavailable or not directly comparable.

### System Tray

Burnly runs with quick access from the system tray or menu bar.

The tray view shows:

- Today's usage
- Today's estimated cost
- Progress toward the current budget
- Recent activity
- A shortcut to open the full dashboard

The tray should remain compact and useful throughout the workday.

### Budgets and Alerts

Users can set optional usage or cost budgets for daily, weekly, and monthly periods.

Burnly can notify users when they:

- Approach a budget threshold
- Reach a budget
- Experience an unusual usage increase

Notifications should be configurable and disabled by default until the user sets a budget.

### History

Burnly maintains a long-term usage history so users can review trends beyond what individual coding tools expose.

Users can compare:

- Today with yesterday
- This week with last week
- This month with last month
- Custom periods

### Data Export

Users can export their selected usage history for personal analysis or record keeping.

Exports must contain only the data described before confirmation.

## First Release

The first release should prove that Burnly can become the easiest way to understand AI coding usage.

### Included

- Automatic discovery of supported coding tools
- Unified usage overview
- Daily, weekly, and monthly date ranges
- Token and estimated-cost summaries
- Breakdown by tool and model
- Activity calendar
- Daily detail view
- System-tray quick view
- Usage history
- Optional budgets and notifications
- Local data storage
- Manual refresh and automatic background refresh
- Data export

### Not included

- User accounts
- Cloud synchronization
- Web dashboard
- Public profiles
- Leaderboards
- Team workspaces
- Organization reporting
- Billing or subscription management
- Prompt, response, or source-code tracking

## Key User Journeys

### First launch

1. The user opens Burnly.
2. Burnly explains what usage information it reads and what it never collects.
3. Burnly finds supported coding tools on the device.
4. The user reviews the detected tools.
5. Burnly presents the initial usage overview.

### Daily check-in

1. The user opens Burnly from the system tray.
2. The user sees today's usage, estimated cost, and budget progress.
3. The user optionally opens the full dashboard for more detail.

### Investigating a usage increase

1. The user notices an unusually active day.
2. The user selects the day in the activity calendar.
3. Burnly shows the tools, models, projects, and sessions that contributed to the total.
4. The user compares the day with their normal activity.

### Setting a budget

1. The user chooses a time period and budget type.
2. The user sets a threshold.
3. Burnly shows progress in the dashboard and tray.
4. Burnly notifies the user at the selected warning levels.

## Privacy Commitments

Burnly's product experience should make these commitments explicit:

- Local use does not require an account.
- Sync is optional and off by default.
- Prompts, responses, source code, and file contents are not collected.
- Users can see what data Burnly stores.
- Users can delete their local history.
- Users control which information is included in exports or future sync.
- Public activity is always opt-in.

Project names and file paths may reveal sensitive information. Burnly should treat them as private and exclude them from future sync unless the user explicitly includes them.

## Success Measures

The first release should be evaluated using:

- Percentage of users who successfully detect at least one coding tool
- Percentage of users who return to check usage after the first day
- Frequency of tray-view usage
- Percentage of users who review the activity calendar
- Percentage of users who configure a budget
- Accuracy and completeness reported by users
- Number of users who continue using Burnly after four weeks

No success measure should reward higher token consumption.

## Product Roadmap

### Phase 1: Local desktop experience

Deliver the complete first-release experience with private local history, a unified dashboard, an activity calendar, tray access, and budgets.

### Phase 2: Personal insights

Add richer comparisons, configurable goals, unusual-activity detection, summaries, and more control over how activity is categorized.

### Phase 3: Optional account and sync

Allow users to create an account and sync selected aggregated usage data across devices. The web dashboard should reflect the same privacy controls as the desktop app.

### Phase 4: Public profile and community

Let users publish selected activity metrics and participate in optional community features.

Public comparison should emphasize consistency and meaningful activity rather than raw token volume. Potential measures include active days, usage streaks, sessions, and projects.

### Phase 5: Teams

Introduce team workspaces, shared budgets, adoption trends, and organization-level reporting with clear employee privacy boundaries.

## Open Product Questions

- Which metric should be the default headline: total tokens, estimated cost, or sessions?
- Should the activity calendar default to tokens or sessions?
- How should Burnly explain subscription-plan usage when monetary cost cannot be estimated accurately?
- What project information is consistently useful without exposing sensitive paths?
- Which budget types provide meaningful control across subscription and usage-based plans?
- Should the tray show combined usage or let users pin a preferred tool?
- What should qualify as an active AI-assisted day for future community features?
- Which activity metrics can be compared fairly across different coding tools?

## Product Positioning

### Short description

Burnly is a private, local-first desktop app for tracking AI coding usage across tools.

### Extended description

Burnly gives developers one place to understand their AI coding activity across tools such as Claude Code, Codex, and OpenCode. It provides daily usage, cost estimates, trends, an activity calendar, budget alerts, and system-tray access without requiring an account.

### Product promise

Understand where your AI coding usage goes, locally and privately.
