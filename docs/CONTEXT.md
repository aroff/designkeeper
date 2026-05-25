# Google Engineering Practices

Google's codified best practices for code review — the collective experience of how to review code and how to get your code through review.

## Language

**CL (Changelist)**:
A single self-contained change submitted to version control and undergoing review.
_Avoid_: change, patch, pull request, diff

**Code Review**:
The process where a **Reviewer** examines a **CL** written by an **Author** before it enters the codebase.
_Avoid_: review (as a standalone noun — use the full term in formal contexts)

**Reviewer**:
A person other than the **Author** who examines a **CL** for correctness, design, and **Code Health**. The Reviewer holds approval authority via LGTM.
_Avoid_: approver, inspector

**Author**:
The person who wrote the **CL** and is seeking approval through **Code Review**.
_Avoid_: developer (used interchangeably in existing docs — prefer Author to distinguish the role from "developer" as a general job title)

**LGTM**:
"Looks Good to Me" — the approval signal from a **Reviewer** indicating the **CL** may be submitted.
_Avoid_: approve, sign-off

**Code Health**:
The overall quality, maintainability, readability, and understandability of the codebase. The supreme metric governing **Code Review** decisions.
_Avoid_: code quality, technical excellence

**Continuous Improvement**:
The principle that a **Reviewer** approves a **CL** once it *definitely improves* **Code Health**, even if it is not perfect. There is no "perfect" code — only "better" code.
_Avoid_: "good enough", minimum viable quality

**Nit**:
A prefix marking a review comment as optional or minor — something that should not block an LGTM.
_Avoid_: minor comment, suggestion

**CL Description**:
A written explanation accompanying a **CL** that tells **Reviewers** what changed and why. A good CL Description replaces the need for Reviewers to read the diff to understand intent.
_Avoid_: commit message, PR description

**Small CL**:
A **CL** that addresses one concern, is easy to review, and contains minimal unrelated changes. Small CLs produce faster and more thorough **Code Review**.
_Avoid_: focused PR, atomic commit

**Emergency**:
A **CL** that must pass **Code Review** as fast as possible due to an ongoing production problem, security hole, or legal issue. Quality standards are relaxed temporarily, with a follow-up review required after submission.
_Avoid_: hotfix, urgent fix (these describe the *cause*, not the *process exception*)

**Pushback**:
**Author** resistance to a **Reviewer's** suggestions. Resolved through polite dialogue, face-to-face discussion, and escalation.
_Avoid_: disagreement, push-back (hyphenated)

**Escalation**:
The conflict resolution path when **Author** and **Reviewer** cannot agree: face-to-face meeting, Tech Lead, code maintainer, Eng Manager.
_Avoid_: escalation path, escalation chain

**Style Guide**:
The authoritative reference for code formatting and naming conventions. Not subject to debate within a **Code Review**.
_Avoid_: linting rules, formatting rules

**Submit**:
The act of entering an approved **CL** into version control.
_Avoid_: check in, check-in, commit, merge

**Review Dimension**:
One of eight quality areas a **Reviewer** evaluates: Design, Functionality, Complexity, Tests, Naming, Comments, Style, Documentation.
_Avoid_: review criteria, quality gate

## Relationships

- An **Author** writes a **CL** and provides a **CL Description**.
- A **Reviewer** examines the **CL** against each **Review Dimension** and either grants LGTM or requests changes.
- A **Nit** is a comment that does not block LGTM.
- **Pushback** occurs when the **Author** disagrees with the **Reviewer**; resolved via **Escalation** if needed.
- An **Emergency** relaxes **Code Review** standards temporarily; a follow-up review is required.
- A **Small CL** accelerates **Code Review** and improves reviewer thoroughness.
- **Submit** occurs only after LGTM (except during an **Emergency**, where timing differs).
- **Continuous Improvement** governs the LGTM threshold: the **CL** must improve **Code Health**, not achieve perfection.

## Example dialogue

> **Dev:** "I have a **CL** that refactors the auth module and also adds a new endpoint. Should I send it as one review?"
>
> **Domain expert:** "Split it into two **Small CLs** — one for the refactor, one for the endpoint. Each gets reviewed on its own merits against the relevant **Review Dimensions**."
>
> **Dev:** "The **Reviewer** left a **Nit** about variable naming but gave LGTM. Do I need to address it before I **Submit**?"
>
> **Domain expert:** "No — a **Nit** is non-blocking by definition. Address it in a follow-up **CL** if you agree, but it shouldn't delay **Submit**."
>
> **Dev:** "What if the **Reviewer** says the design is over-engineered and I disagree?"
>
> **Domain expert:** "That's **Pushback**. Start with a face-to-face conversation. If you still can't agree, follow the **Escalation** path — Tech Lead first. The **Continuous Improvement** principle applies: the **CL** needs to improve **Code Health**, not be perfect, but over-engineering harms **Code Health** too."

## Flagged ambiguities

- **"Developer" vs. "Author"**: The directory is named `review/developer/` but the guides themselves say "CL Author." This project uses **Author** as the canonical term for the person writing the CL. "Developer" is a job title — not a review role. The directory name is a legacy artifact.
- **"Submit" vs. "Check in"**: Both appear in the docs as synonyms for entering code into version control. **Submit** is canonical.
- **"Review" vs. "Code Review"**: "Review" is acceptable as shorthand once the context is established, but **Code Review** should be used in headings, formal definitions, and the first mention in any document.
- **"g3doc"** and **"OWNERS file"**: Google-specific terms used in examples without definition. These are internal artifacts and should remain as-is in examples but should not appear in general guidance.
