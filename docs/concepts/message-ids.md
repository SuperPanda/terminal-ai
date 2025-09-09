# Message ID Scheme: Audience Concept via Parent Message ID

## 1. Message ID Definition

The Message ID is designed to capture both the speech identity and the audience identity through a structured hash-based approach. It consists of two components:

- **Speech Identity**: `HASH(TIMESTAMP + SPEAKER)`
  - Uniquely identifies the speech act based on the timestamp (time of utterance) and the speaker (resource that produced the message)
  - Ensures that each speech act has a unique origin point, even if content is similar

- **Audience Identity**: `HASH(PARENT_MESSAGE_ID)`
  - Represents the message that prompted this response (the audience)
  - For a root message, the parent is `NULL`, which is hashed consistently to form an initial object

The full Message ID is formatted as: `[Speech Identity]-[Audience Identity]` (e.g., `a3659d9c-21726c10`).

## 2. Audience Concept: Parent Message ID as Audience

The audience is not explicitly stored as a separate entity but is implied through the parent message ID:

- **Parent Message ID**: Points to the message that was "heard" or "observed" by the speaker, triggering the response
- **Root Messages**: If no message was observed (e.g., the first message in a conversation), the parent is `NULL`
- **Category Theory Perspective**: The parent ID defines the morphism from the audience message to the current message
- **Thought Support**: When a speaker responds to itself (internal thought), the parent ID points to its own previous message, creating a self-referential morphism

## 3. Examples of Message IDs

### Root Message (No Audience)
```
Speaker: SPEAKER_A
Timestamp: T1
Parent: NULL
Speech Identity: HASH(T1 + SPEAKER_A) → a3659d9c
Audience Identity: HASH(NULL) → 21726c10
Message ID: a3659d9c-21726c10
```

### Response to Root Message
```
Speaker: SPEAKER_B
Timestamp: T2
Parent ID: a3659d9c-21726c10
Speech Identity: HASH(T2 + SPEAKER_B) → 6b86b273
Audience Identity: HASH(a3659d9c-21726c10) → 366d16a1
Message ID: 6b86b273-366d16a1
```

### Self-Response (Thought)
```
Speaker: SPEAKER_A
Timestamp: T3
Parent ID: a3659d9c-21726c10
Speech Identity: HASH(T3 + SPEAKER_A) → 9d0c83bd
Audience Identity: HASH(a3659d9c-21726c10) → 366d16a1
Message ID: 9d0c83bd-366d16a1
```

## 4. Key Properties Supported

- **Coherence with Multiple Responses**: Multiple messages can respond to the same parent
- **Coherence with Talking to Self**: Self-responses are captured using same speaker with different timestamps
- **Morphism Tracking**: Audience identity enables backward traversal of conversation graph
- **Unique Morphisms**: Initial object (NULL parent) ensures unique morphisms from root
- **Support for Multiple Agents**: Different speakers can respond to the same message

## 5. Handling Streaming and Chunks

For streaming scenarios where a speech act is delivered in chunks:

- **Same Speech Identity**: All chunks share the same timestamp and speaker
- **Consistent Audience Identity**: All chunks respond to the same parent message
- **Efficient Retrieval**: Chunks can be grouped by Message ID for reconstruction

## 6. Refinements and Notes

- **TIMESTAMP**: Must be precise enough (e.g., microseconds) to ensure uniqueness
- **SPEAKER**: Identifies the resource or agent, enabling multi-entity responses
- **PARENT**: Captures audience morphism without storing explicit audience details
- **NULL Handling**: Hashed consistently (e.g., `HASH("NULL")`) for root messages

This scheme ensures that messages are defined by their relationship to an audience (parent message), enabling a robust category theory-based model for conversations.

---
*Document Version: 0.0.1*  
*Last Updated: Tue 26 Aug 2025 06:09:37*  
