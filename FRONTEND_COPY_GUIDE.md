## Interface language standard

### Core rule
**Short labels. Clear headings. Contextual help only when needed. Detailed explanations on demand.**

### Copy hierarchy
1. **Primary:** concise page title or metric label that identifies the content.
2. **Secondary:** one short sentence only when it helps users understand the purpose or next step.
3. **Contextual:** definitions, methodology, calculation assumptions, caveats, and UTC explanations in an info tooltip, popover, expandable details, or a nearby help link.
4. **Reference:** full technical/statistical explanations in dedicated documentation or a methodology view—not repeated on every page.

### Writing rules
- Prefer familiar words over internal implementation terminology.
- Use sentence case for headings and labels; reserve uppercase for compact technical metadata where it is already an established visual convention.
- Use concise, action-oriented button labels: “Retry”, “Connect account”, “View details”, “Create journal”.
- Avoid repeating the same explanation in a page subtitle, card description, alert, and tooltip.
- Do not expose internal enum values, snake_case identifiers, raw API names, or machine-oriented group keys as user-facing labels. Map display labels only; preserve canonical stored values and grouping behavior.
- Format labels consistently: e.g. `london` → “London”, `london_ny_overlap` → “London–New York overlap”, `new_york` → “New York”.
- Show timezone context once at the relevant section/filter level (e.g. “Times shown in UTC”), not repeated in every row or chart.
- Keep timestamps compact in the interface; provide exact generation time in metadata or details where auditability requires it.
- Keep privacy-sensitive identity information out of public pages and avoid repeating email addresses unless the user needs them for account management.
- Do not simplify away important distinctions between Journal and broker execution data, currencies, stale data, sample sizes, paper/live modes, or risk-policy outcomes.

### Contextual help placement
- Use an info icon with an accessible label for definitions and methodology.
- Use a tooltip/popover for short definitions; use an expandable “How this is calculated” or “Methodology” section for longer explanations.
- Keep critical warnings, stale/disconnected/unknown states, required acknowledgements, and material financial limitations visible. They must not be hidden in a tooltip.
- Tooltips must be keyboard-accessible and not hover-only.
- Avoid help icons on obvious labels; help should answer a real likely question.

### Before / after examples
| Avoid | Prefer |
|---|---|
| TraderOS-style position sizing with backend FX conversion and account risk policies | Position sizing & risk planning |
| Understand your trading edge across profitability, execution, behaviour and risk. | Understand your trading performance. |
| Signed in as user@example.com. | Keep identity in the account menu. |
| Analytics snapshot generated [timestamp] · All grouped session, weekday and hour metrics use UTC. | Updated [time] UTC · Times grouped by UTC |
| Statistical context, not a prediction. The interval communicates uncertainty in the observed win rate; it is not a probability that the true rate lies inside these bounds. | Win-rate confidence interval · Not a prediction. Put the full statistical interpretation in methodology help. |

These are copy/display examples only. They do not authorize changes to formulas, stored values, timestamps, grouping keys, or risk semantics.


