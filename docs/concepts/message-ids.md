# Message ID Specification

**Version:** 0.0.3
**Status:** Draft

## Overview

Messages are identified by their relationship to prior speech.

Each message captures:
- **Speech identity**: who spoke and when
- **Audience identity**: what speech prompted this response

## Definitions

### Speech

A speech is a speaker at a specific time. Not what was said, but who spoke and when.

```
Speech := (Speaker, Timestamp)
```

Two particles cannot occupy the same space at the same time.
Two speeches cannot share the same `(Speaker, Timestamp)` pair.

**Speech Identity:**
```
SpeechID := HASH8(Speaker, Timestamp)
```

Where `HASH8` produces 8 hexadecimal characters from SHA-256. Content is attached metadata, not part of identity.

### Message

A message is a speech together with what it responds to.

```
Message := (Speech, ParentSpeech)
```

Where `ParentSpeech` is:
- The SpeechID of another speech (what prompted this), or
- `NULL` for root utterances

**Audience Identity:**

The audience is what is responded to—the parent speech:
```
AudienceID := SpeechID(parent_speech)     if replying
           := HASH8("NULL")              if root
```

**Message Identity:**
```
MessageID := "<SpeechID>-<AudienceID>"
```

The structure `speech-audience` encodes a morphism from audience to speech.

## Examples

### Root Message

```
Speaker:    USER
Timestamp:  T1
Parent:     NULL

SpeechID    = HASH8(USER, T1)      → a3659d9c
AudienceID  = HASH8("NULL")        → 21726c10
MessageID   = a3659d9c-21726c10
```

### Response

```
Speaker:    AI
Timestamp:  T2
Parent:     a3659d9c-21726c10

SpeechID    = HASH8(AI, T2)        → 6b86b273
AudienceID  = a3659d9c             (parent's SpeechID)
MessageID   = 6b86b273-a3659d9c
```

### Self-Response

```
Speaker:    USER
Timestamp:  T3
Parent:     a3659d9c-21726c10

SpeechID    = HASH8(USER, T3)      → 9d0c83bd
AudienceID  = a3659d9c
MessageID   = 9d0c83bd-a3659d9c
```

## Structure

Messages form a directed graph:
- **Nodes**: Speech identities (who-when pairs)
- **Edges**: Messages connecting parent speech to current speech

Each MessageID encodes one edge:
```
MessageID = S_current - S_parent
```

Read as: "current speech responding to parent speech"

```
HASH8("NULL") → root_speech → response_speech → continuation
                    ↓
                  branch_speech
```

Multiple messages may share the same `AudienceID`, creating branches from a single parent speech. This is not a conflict—it's multiple responses to the same prompt.

### Conversation Threads

A thread is a path through the graph:
```
s₀ → s₁ → s₂ → s₃
```

Where each arrow is a message whose `AudienceID` equals the prior message's `SpeechID`.

### Self-Chains

When `Speaker(sᵢ) = Speaker(sᵢ₊₁)`, the speaker responds to themselves:
```
USER[T1] → USER[T2] → USER[T3]
```

This captures internal thought or continued monologue. Different snapshots of the same entity.

## Properties

**Composition:** Messages compose by chaining. If message M₁ has `AudienceID = S₀` and `SpeechID = S₁`, and message M₂ has `AudienceID = S₁`, then M₂ responds to M₁.

**Self-reference:** When `Speaker` is constant across messages, the graph represents internal thought or continued monologue.

**Roots:** All conversation threads trace back to messages with `AudienceID = HASH8("NULL")`.

**Determinism:** Given the same `(Speaker, Timestamp)` pair, `SpeechID` is invariant. Given the same parent, `AudienceID` is invariant.

## Notes

- Timestamp precision should be sufficient to prevent collisions (seconds are typically adequate; microseconds available if needed)
- Speaker can be any identifier: `USER`, `AI`, `AI:model-name`, `TOOL:name`
- Content is separate from identity; changing content doesn't change the message's structural position
- Parallel responses to the same parent create distinct branches, not conflicts
- **Critical distinction:** `AudienceID` is the *SpeechID of the parent speech*, not a hash of the parent's full MessageID. This makes morphisms explicit and enables direct graph traversal.
