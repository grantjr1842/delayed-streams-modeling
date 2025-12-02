# Next.js Real-Time Speech-to-Text Web Client

## Systems Design Document

**Version:** 2.0.0  
**Date:** December 2024  
**Status:** Final - Agent-Ready  
**Reference Implementation:** `moshi/client/`  
**Related Docs:** [BETTER_AUTH_INTEGRATION.md](./BETTER_AUTH_INTEGRATION.md), [MOSHI_SERVER_SETUP.md](./MOSHI_SERVER_SETUP.md)

---

## 🤖 Agent Implementation Guide

> **This section is designed for AI coding agents.** It provides explicit, ordered instructions for implementing the complete project from scratch.

### Prerequisites Checklist

Before starting implementation, verify:

- [ ] Node.js 20+ installed
- [ ] pnpm installed (`npm install -g pnpm`)
- [ ] Empty repository created for the project
- [ ] Access to `moshi/client/public/assets/` for decoder files

### Phase 0: Project Initialization (Run These Commands First)

```bash
# 1. Initialize Next.js project (run in empty repo root)
pnpm create next-app@latest ./ \
  --typescript \
  --react-compiler \
  --tailwind \
  --biome \
  --app \
  --no-src-dir \
  --rspack \
  --import-alias "@/*" \
  --use-pnpm \
  --yes

# 2. Initialize shadcn/ui
pnpm dlx shadcn@latest init -y

# 3. Install all shadcn/ui components (single command)
pnpm dlx shadcn@latest add button card dialog input label select slider switch badge tooltip sonner scroll-area separator skeleton progress tabs alert alert-dialog dropdown-menu sheet popover command avatar collapsible -y

# 4. Install runtime dependencies
pnpm add zustand nanoid opus-recorder webm-duration-fix better-auth @tanstack/react-query zod server-only client-only next-themes

# 5. Install dev dependencies
pnpm add -D vitest @vitejs/plugin-react jsdom @testing-library/react playwright @playwright/test

# 6. Copy decoder assets (adjust source path as needed)
mkdir -p public/assets
cp ../moshi/client/public/assets/decoderWorker.min.js public/assets/
cp ../moshi/client/public/assets/decoderWorker.min.wasm public/assets/
```

### Phase 1: File Creation Order (Critical - Follow Exactly)

Files MUST be created in this order due to import dependencies:

#### Layer 1: Types & Utilities (No Dependencies)

| Order | File Path | Description |
|-------|-----------|-------------|
| 1.1 | `types/audio.ts` | Audio-related TypeScript types |
| 1.2 | `types/transcript.ts` | Transcript message types |
| 1.3 | `types/websocket.ts` | WebSocket message types |
| 1.4 | `types/settings.ts` | Settings types |
| 1.5 | `lib/utils/cn.ts` | Tailwind class merge utility |
| 1.6 | `lib/utils/format-time.ts` | Time formatting utilities |
| 1.7 | `lib/utils/debounce.ts` | Debounce utility |
| 1.8 | `lib/audio/constants.ts` | Audio constants (sample rate, buffer size) |
| 1.9 | `lib/env.ts` | Environment variable validation |

#### Layer 2: Protocol & Core Libraries (Depends on Layer 1)

| Order | File Path | Description |
|-------|-----------|-------------|
| 2.1 | `lib/protocol/types.ts` | Protocol message type definitions |
| 2.2 | `lib/protocol/encoder.ts` | Binary protocol encoder/decoder |
| 2.3 | `lib/websocket/types.ts` | WebSocket client types |
| 2.4 | `lib/websocket/security.ts` | URL validation, auth helpers |
| 2.5 | `lib/websocket/client.ts` | STTWebSocketClient class |
| 2.6 | `lib/audio/audio-context.ts` | AudioContext factory (client-only) |
| 2.7 | `lib/decoder/decoderWorker.ts` | Opus decoder worker loader |
| 2.8 | `lib/auth/types.ts` | Auth type definitions |
| 2.9 | `lib/auth/auth-client.ts` | Better Auth client config |

#### Layer 3: State Management (Depends on Layer 2)

| Order | File Path | Description |
|-------|-----------|-------------|
| 3.1 | `lib/stores/audio-store.ts` | Audio state (Zustand) |
| 3.2 | `lib/stores/transcript-store.ts` | Transcript state (Zustand) |
| 3.3 | `lib/stores/connection-store.ts` | Connection state (Zustand) |

#### Layer 4: Hooks (Depends on Layer 3)

| Order | File Path | Description |
|-------|-----------|-------------|
| 4.1 | `hooks/use-local-storage.ts` | localStorage persistence |
| 4.2 | `hooks/use-audio-devices.ts` | Device enumeration |
| 4.3 | `hooks/use-user-audio.ts` | Microphone capture with opus-recorder |
| 4.4 | `hooks/use-audio-capture.ts` | Audio capture orchestration |
| 4.5 | `hooks/use-websocket.ts` | WebSocket connection hook |
| 4.6 | `hooks/use-transcript.ts` | Transcript state hook |
| 4.7 | `hooks/use-auth.ts` | Authentication hook |

#### Layer 5: UI Components (Depends on Layer 4)

| Order | File Path | Description |
|-------|-----------|-------------|
| 5.1 | `components/providers/theme-provider.tsx` | Theme provider wrapper |
| 5.2 | `components/providers/query-provider.tsx` | React Query provider |
| 5.3 | `components/providers/providers.tsx` | Combined providers |
| 5.4 | `components/layout/theme-toggle.tsx` | Theme toggle button |
| 5.5 | `components/layout/footer.tsx` | Footer (Server Component) |
| 5.6 | `components/connection/connection-status.tsx` | Status badge |
| 5.7 | `components/connection/server-config.tsx` | Config dialog |
| 5.8 | `components/audio/vad-indicator.tsx` | VAD status |
| 5.9 | `components/audio/device-selector.tsx` | Mic selector |
| 5.10 | `components/audio/audio-meter.tsx` | Level meter |
| 5.11 | `components/audio/audio-controls.tsx` | Record buttons |
| 5.12 | `components/transcript/partial-text.tsx` | Live text |
| 5.13 | `components/transcript/transcript-line.tsx` | Final text line |
| 5.14 | `components/transcript/transcript-panel.tsx` | Main panel |
| 5.15 | `components/layout/header.tsx` | Header with nav |

#### Layer 6: Pages & API Routes (Depends on Layer 5)

| Order | File Path | Description |
|-------|-----------|-------------|
| 6.1 | `app/globals.css` | Global styles with theme |
| 6.2 | `app/layout.tsx` | Root layout |
| 6.3 | `app/page.tsx` | Home page |
| 6.4 | `app/api/health/route.ts` | Health check API |
| 6.5 | `app/api/config/route.ts` | Config API |
| 6.6 | `app/(routes)/settings/page.tsx` | Settings page |
| 6.7 | `app/(routes)/history/page.tsx` | History page |

#### Layer 7: Configuration Files

| Order | File Path | Description |
|-------|-----------|-------------|
| 7.1 | `.env.example` | Environment template |
| 7.2 | `tailwind.config.ts` | Tailwind with custom colors |
| 7.3 | `middleware.ts` | Security headers |
| 7.4 | `public/worklets/audio-processor.js` | AudioWorklet script |

### Essential Constants Reference

```typescript
// lib/audio/constants.ts
export const AUDIO_CONSTANTS = {
  SAMPLE_RATE: 24000,           // Target sample rate for server
  BUFFER_LENGTH: 960,           // Samples per buffer (960 = 24000 / 12.5 / 2)
  ENCODER_FRAME_SIZE: 20,       // Opus frame size in ms
  MAX_FRAMES_PER_PAGE: 2,       // Low latency setting
  CHANNELS: 1,                  // Mono audio
  ENCODER_APPLICATION: 2049,    // OPUS_APPLICATION_VOIP
} as const;

export const WS_CONSTANTS = {
  MAX_RECONNECT_ATTEMPTS: 5,
  RECONNECT_DELAY_MS: 1500,
  INACTIVITY_TIMEOUT_MS: 10000,
} as const;
```

### Binary Protocol Quick Reference

```typescript
// Message type bytes (first byte of each message)
const MESSAGE_TYPES = {
  HANDSHAKE: 0x00,    // [version, model]
  AUDIO: 0x01,        // [opus_data...]
  TEXT: 0x02,         // [utf8_text...]
  CONTROL: 0x03,      // [action] (0=start, 1=endTurn, 2=pause, 3=restart)
  METADATA: 0x04,     // [json_utf8...]
  ERROR: 0x05,        // [utf8_message...]
  PING: 0x06,         // (no payload)
  COLORED_TEXT: 0x07, // [color, utf8_text...]
} as const;
```

### Verification Commands

After each phase, run these to verify:

```bash
# After Phase 0
pnpm build  # Should complete without errors

# After Layer 1-3
pnpm tsc --noEmit  # Type check

# After Layer 4-5
pnpm dev  # Start dev server, check for import errors

# After Layer 6-7
pnpm build && pnpm start  # Full production build
```

### Common Pitfalls & Solutions

| Problem | Cause | Solution |
|---------|-------|----------|
| `'use client'` error in Server Component | Importing client hook in server file | Move hook usage to a Client Component wrapper |
| `window is not defined` | Browser API used during SSR | Wrap in `useEffect` or use `'use client'` |
| `Module not found: @/lib/...` | File not created yet | Follow Layer order exactly |
| AudioWorklet not loading | Wrong path or CORS | Ensure file is in `public/worklets/` |
| WebSocket auth fails | Missing token in URL | Check `buildAuthenticatedWsUrl()` |
| Opus encoder error | Missing WASM files | Copy decoder assets to `public/assets/` |
| Hydration mismatch | Theme/state differs server vs client | Use `mounted` state pattern |
| `clsx` not found | Missing dependency | Run `pnpm add clsx tailwind-merge` |

### Implementation Completion Checklist

Use this to verify all critical files exist:

```bash
# Run this script to check file existence
files=(
  "lib/utils/cn.ts"
  "lib/audio/constants.ts"
  "lib/protocol/encoder.ts"
  "lib/websocket/client.ts"
  "lib/stores/audio-store.ts"
  "lib/stores/transcript-store.ts"
  "lib/stores/connection-store.ts"
  "lib/auth/auth-client.ts"
  "hooks/use-user-audio.ts"
  "hooks/use-websocket.ts"
  "hooks/use-auth.ts"
  "components/providers/providers.tsx"
  "components/audio/audio-controls.tsx"
  "components/transcript/transcript-panel.tsx"
  "components/layout/header.tsx"
  "app/layout.tsx"
  "app/page.tsx"
  "public/assets/decoderWorker.min.js"
  "public/assets/decoderWorker.min.wasm"
)

for f in "${files[@]}"; do
  [ -f "$f" ] && echo "✓ $f" || echo "✗ $f MISSING"
done
```

### Key Integration Points

When implementing, pay special attention to these integration points:

1. **WebSocket ↔ Transcript Store**: Messages from `STTWebSocketClient.onMessage` must update `transcript-store`
2. **Audio Capture ↔ WebSocket**: `useUserAudio.onDataChunk` must call `sendMessage({ type: 'audio', data })`
3. **Auth ↔ WebSocket**: `useAuth.getSessionToken()` must be passed to WebSocket URL builder
4. **Stores ↔ Components**: All UI components subscribe to stores via hooks, not props
5. **Theme ↔ Layout**: `ThemeProvider` must wrap entire app in `layout.tsx`

---

## Table of Contents

1. [🤖 Agent Implementation Guide](#-agent-implementation-guide) ⬆️ *Start Here*
2. [Quick Reference](#quick-reference)
3. [Executive Summary](#executive-summary)
4. [User Stories & Personas](#user-stories--personas)
5. [System Architecture](#system-architecture)
6. [Technical Requirements](#technical-requirements)
7. [Frontend Architecture](#frontend-architecture)
8. [Backend Architecture](#backend-architecture)
9. [WebSocket Protocol Integration](#websocket-protocol-integration)
10. [Audio Pipeline Design](#audio-pipeline-design)
11. [UI/UX Design](#uiux-design)
12. [Security Considerations](#security-considerations)
13. [Performance Optimization](#performance-optimization)
14. [Testing Strategy](#testing-strategy)
15. [Deployment Strategy](#deployment-strategy)
16. [GitHub Issues & Branches](#github-issues--branches)
17. [Implementation Timeline](#implementation-timeline)
18. [Existing Moshi Client Implementation Reference](#existing-moshi-client-implementation-reference)

---

## Quick Reference

> **Copy-paste ready snippets for common implementation tasks.**

### Minimal Working Store (Zustand Template)

```typescript
// lib/stores/[name]-store.ts
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface ExampleState {
  value: string;
  setValue: (v: string) => void;
}

export const useExampleStore = create<ExampleState>()(
  persist(
    (set) => ({
      value: '',
      setValue: (value) => set({ value }),
    }),
    { name: 'example-storage' }
  )
);
```

### Minimal Client Component Template

```tsx
// components/example/example.tsx
'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils/cn';

interface ExampleProps {
  className?: string;
}

export function Example({ className }: ExampleProps) {
  const [state, setState] = useState(false);
  
  return (
    <div className={cn('', className)}>
      <Button onClick={() => setState(!state)}>
        Toggle
      </Button>
    </div>
  );
}
```

### Minimal Server Component Template

```tsx
// components/example/server-example.tsx
// NO 'use client' directive - this is a Server Component

import { cn } from '@/lib/utils/cn';

interface ServerExampleProps {
  data: string;
  className?: string;
}

export function ServerExample({ data, className }: ServerExampleProps) {
  return (
    <div className={cn('', className)}>
      {data}
    </div>
  );
}
```

### Minimal API Route Template

```typescript
// app/api/example/route.ts
import { NextRequest, NextResponse } from 'next/server';

export async function GET(request: NextRequest): Promise<NextResponse> {
  return NextResponse.json({ status: 'ok' });
}

export async function POST(request: NextRequest): Promise<NextResponse> {
  const body = await request.json();
  return NextResponse.json({ received: body }, { status: 201 });
}
```

### Minimal Hook Template

```typescript
// hooks/use-example.ts
'use client';

import { useState, useCallback, useEffect } from 'react';

export function useExample() {
  const [state, setState] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const doSomething = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      // async operation
      setState('result');
    } catch (err) {
      setError(err instanceof Error ? err : new Error('Unknown error'));
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { state, isLoading, error, doSomething };
}
```

### cn() Utility (Required First)

```typescript
// lib/utils/cn.ts
import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

### Environment Variables (.env.example)

```env
# Required
NEXT_PUBLIC_APP_URL=http://localhost:3000
DEFAULT_STT_URL=wss://stt.fullen.dev/api/asr-streaming

# Optional - Authentication
NEXT_PUBLIC_AUTH_URL=
BETTER_AUTH_SECRET=

# Optional - Features
FEATURE_HISTORY=true
FEATURE_EXPORT=true

# Optional - Database
DATABASE_URL=
```

### Import Path Conventions

| Import | Resolves To |
|--------|-------------|
| `@/components/*` | `./components/*` |
| `@/lib/*` | `./lib/*` |
| `@/hooks/*` | `./hooks/*` |
| `@/types/*` | `./types/*` |
| `@/app/*` | `./app/*` |

---

## Executive Summary

This document outlines the complete development of a Next.js web application that streams microphone audio to the Kyutai Rust STT server and displays real-time transcriptions. The application will leverage the Web Audio API for audio capture, WebSocket connections for bidirectional communication, and modern React patterns for responsive UI updates.

### Key Features

- **Real-time microphone streaming** via Web Audio API
- **WebSocket-based communication** with the Rust STT server
- **Live transcription display** with partial and final results
- **Voice Activity Detection (VAD)** visualization
- **Secure connections** (WSS) with API key authentication
- **Responsive, accessible UI** built with shadcn/ui and Tailwind CSS

---

## User Stories & Personas

### User Personas

#### 1. Alex - Content Creator / Podcaster
**Background:** Alex is a content creator who produces podcasts and video content. They need accurate transcriptions for show notes, SEO optimization, and accessibility compliance.

**Goals:**
- Quickly transcribe podcast recordings and live sessions
- Export transcripts in multiple formats for different platforms
- Minimize manual editing of transcriptions

**Pain Points:**
- Existing transcription services are expensive for long-form content
- Batch processing takes too long; needs real-time feedback
- Poor accuracy with technical terminology and proper nouns

**Technical Proficiency:** Intermediate - comfortable with web apps, basic understanding of audio settings

---

#### 2. Dr. Sarah Chen - Medical Professional
**Background:** Dr. Chen is a physician who needs to document patient consultations efficiently while maintaining eye contact and rapport with patients.

**Goals:**
- Hands-free transcription during consultations
- Secure, HIPAA-compliant data handling
- Quick review and editing of transcripts

**Pain Points:**
- Typing during consultations breaks patient rapport
- Voice recognition often fails with medical terminology
- Concerns about data privacy and security

**Technical Proficiency:** Basic - prefers simple, intuitive interfaces

---

#### 3. Marcus - Software Developer
**Background:** Marcus is a developer who wants to integrate speech-to-text capabilities into his applications and needs to test/evaluate the STT server.

**Goals:**
- Test STT server performance and accuracy
- Understand the WebSocket protocol and message formats
- Debug connection issues and audio pipeline problems

**Pain Points:**
- Lack of visibility into audio processing pipeline
- Difficulty diagnosing connection and encoding issues
- Need for detailed metrics and debugging information

**Technical Proficiency:** Expert - comfortable with developer tools, WebSocket debugging, audio APIs

---

#### 4. Jamie - Student with Accessibility Needs
**Background:** Jamie is a university student with hearing impairment who uses real-time transcription for lectures and meetings.

**Goals:**
- Real-time captions with minimal latency
- Clear, readable transcript display
- Ability to review and search past transcriptions

**Pain Points:**
- Existing solutions have too much latency for real-time use
- Small text and poor contrast make reading difficult
- No way to catch up on missed content

**Technical Proficiency:** Intermediate - daily technology user, familiar with accessibility tools

---

#### 5. Corporate IT Admin - Taylor
**Background:** Taylor manages technology deployments for a mid-size company and needs to evaluate and deploy transcription solutions.

**Goals:**
- Evaluate security and compliance features
- Configure server connections for corporate environment
- Monitor usage and system health

**Pain Points:**
- Solutions that require cloud connectivity are blocked by firewall
- Need for on-premise deployment options
- Lack of enterprise configuration options

**Technical Proficiency:** Advanced - experienced with enterprise software deployment

---

### Epic 1: First-Time User Experience

#### User Story 1.1: Landing Page Introduction
**As a** first-time visitor  
**I want to** understand what this application does immediately  
**So that** I can decide if it meets my needs

**Acceptance Criteria:**
- [ ] Landing page clearly explains the application's purpose within 5 seconds
- [ ] Key features are highlighted with icons and brief descriptions
- [ ] "Get Started" button is prominently displayed above the fold
- [ ] Demo video or animated GIF shows the application in action
- [ ] No login required to view the landing page

**Scenario:**
```gherkin
Given I am a new visitor to the application
When I load the homepage
Then I should see a clear headline explaining "Real-time Speech-to-Text"
And I should see at least 3 key feature highlights
And I should see a prominent "Get Started" or "Try Now" button
And the page should load in under 2 seconds
```

---

#### User Story 1.2: Microphone Permission Request
**As a** new user  
**I want to** understand why microphone access is needed before granting permission  
**So that** I feel confident about my privacy

**Acceptance Criteria:**
- [ ] Clear explanation appears before browser permission prompt
- [ ] Privacy policy link is visible and accessible
- [ ] User can proceed without granting permission (with limited functionality)
- [ ] Permission denial is handled gracefully with helpful guidance
- [ ] No audio is captured until explicit user action (clicking record)

**Scenario:**
```gherkin
Given I am a new user who has not granted microphone permission
When I click the "Start Recording" button
Then I should see an explanation modal before the browser permission prompt
And the modal should explain that audio is processed locally/on-server
And I should see a link to the privacy policy
And I should be able to dismiss the modal without granting permission

Given I deny microphone permission
When the permission prompt closes
Then I should see a helpful message explaining how to enable it later
And the application should remain functional for viewing past transcripts
```

---

#### User Story 1.3: Server Configuration for First Use
**As a** new user  
**I want to** easily connect to a speech-to-text server  
**So that** I can start transcribing immediately

**Acceptance Criteria:**
- [ ] Default server URL is pre-configured if available
- [ ] Server connection status is clearly visible
- [ ] Invalid URLs show helpful error messages
- [ ] Connection test button validates server reachability
- [ ] API key field is available but optional (depending on server config)
- [ ] Settings are persisted in localStorage for return visits

**Scenario:**
```gherkin
Given I am configuring the application for the first time
When I open the settings panel
Then I should see the server URL field pre-populated with a default value
And I should see a "Test Connection" button
And I should see the current connection status

Given I enter an invalid server URL
When I click "Test Connection"
Then I should see a specific error message (e.g., "Invalid URL format" or "Server unreachable")
And the error should suggest how to fix the issue

Given I enter a valid server URL
When I click "Test Connection"
Then I should see a success message with server latency
And the connection status should update to "Connected"
```

---

### Epic 2: Core Transcription Workflow

#### User Story 2.1: Start Recording
**As a** user  
**I want to** start recording my voice with a single click  
**So that** I can begin transcription quickly

**Acceptance Criteria:**
- [ ] Large, clearly visible "Record" button
- [ ] Visual feedback when recording starts (button state change, animation)
- [ ] Audio level meter shows microphone input is being captured
- [ ] Keyboard shortcut available (Ctrl/Cmd + Space)
- [ ] Recording starts within 100ms of button click
- [ ] Error state if microphone is unavailable

**Scenario:**
```gherkin
Given I have granted microphone permission
And I am connected to the STT server
When I click the "Record" button
Then the button should change to a "Stop" state with visual indicator
And the audio meter should show my voice level
And I should see "Recording..." status text
And the WebSocket should begin receiving audio data

Given I press Ctrl+Space
When the shortcut is triggered
Then recording should toggle on/off
And I should see a brief toast notification confirming the action
```

---

#### User Story 2.2: View Real-Time Transcription
**As a** user  
**I want to** see my speech transcribed in real-time  
**So that** I can verify the transcription accuracy as I speak

**Acceptance Criteria:**
- [ ] Partial transcriptions appear within 500ms of speaking
- [ ] Partial text is visually distinct from final text (e.g., italic, different color)
- [ ] Final transcriptions replace partial text smoothly
- [ ] Transcript panel auto-scrolls to show latest content
- [ ] Timestamps are displayed for each segment
- [ ] Confidence indicators show transcription reliability

**Scenario:**
```gherkin
Given I am recording audio
When I speak "Hello, this is a test"
Then I should see partial text appear within 500ms
And the partial text should be styled differently (e.g., italic, gray)
And when the server sends a final transcription
Then the partial text should be replaced with final text
And the final text should include a timestamp
And the transcript panel should auto-scroll to show the new text

Given the transcription has low confidence (< 70%)
When the final text is displayed
Then I should see a visual indicator (e.g., yellow highlight, warning icon)
And hovering over the text should show the confidence percentage
```

---

#### User Story 2.3: Stop Recording
**As a** user  
**I want to** stop recording when I'm finished  
**So that** I can review and export my transcript

**Acceptance Criteria:**
- [ ] "Stop" button is clearly visible during recording
- [ ] Stopping recording flushes any remaining audio to the server
- [ ] Final transcriptions are received before session ends
- [ ] Recording duration is displayed
- [ ] Option to save transcript is presented after stopping

**Scenario:**
```gherkin
Given I am currently recording
When I click the "Stop" button
Then the button should return to "Record" state
And any pending audio should be sent to the server
And I should see a brief "Processing..." indicator
And when all final transcriptions are received
Then I should see a "Save Transcript" prompt
And the total recording duration should be displayed
```

---

#### User Story 2.4: Pause and Resume Recording
**As a** user  
**I want to** pause recording temporarily without ending my session  
**So that** I can take breaks without losing context

**Acceptance Criteria:**
- [ ] "Pause" button available during recording
- [ ] Paused state is clearly indicated visually
- [ ] Audio is not captured or sent while paused
- [ ] Resume continues the same transcript session
- [ ] Pause duration is tracked but not included in recording time

**Scenario:**
```gherkin
Given I am currently recording
When I click the "Pause" button
Then the button should change to "Resume"
And the audio meter should show no activity
And a "Paused" indicator should be visible
And no audio data should be sent to the server

Given recording is paused
When I click the "Resume" button
Then recording should continue
And new transcriptions should append to the existing transcript
And the pause duration should be noted in the transcript metadata
```

---

### Epic 3: Transcript Management

#### User Story 3.1: Copy Transcript to Clipboard
**As a** user  
**I want to** copy my transcript to the clipboard  
**So that** I can paste it into other applications

**Acceptance Criteria:**
- [ ] "Copy" button is easily accessible
- [ ] Copies only final transcriptions (not partial)
- [ ] Success feedback (toast notification)
- [ ] Keyboard shortcut available (Ctrl/Cmd + C when transcript is focused)
- [ ] Option to copy with or without timestamps

**Scenario:**
```gherkin
Given I have a transcript with multiple segments
When I click the "Copy" button
Then the transcript text should be copied to my clipboard
And I should see a "Copied!" toast notification
And the copied text should only include final transcriptions

Given I want to copy with timestamps
When I click the "Copy" dropdown and select "Copy with timestamps"
Then each line should be prefixed with its timestamp
And the format should be "[HH:MM:SS] Text content"
```

---

#### User Story 3.2: Export Transcript
**As a** user  
**I want to** export my transcript in various formats  
**So that** I can use it in different applications and workflows

**Acceptance Criteria:**
- [ ] Export formats: Plain text (.txt), JSON, SRT subtitles, VTT subtitles
- [ ] Export button with format dropdown
- [ ] File downloads immediately after selection
- [ ] Filename includes date and optional custom title
- [ ] Large transcripts export without browser freezing

**Scenario:**
```gherkin
Given I have a completed transcript
When I click the "Export" button
Then I should see a dropdown with format options: TXT, JSON, SRT, VTT
And each option should have a brief description

Given I select "SRT" format
When the export processes
Then a file should download with .srt extension
And the file should contain properly formatted SRT subtitles
And each segment should have sequential numbering and timestamps

Given I select "JSON" format
When the export processes
Then the file should contain the full transcript data
And it should include metadata (duration, word count, server info)
And it should be valid, parseable JSON
```

---

#### User Story 3.3: Clear Current Transcript
**As a** user  
**I want to** clear my current transcript  
**So that** I can start fresh without reloading the page

**Acceptance Criteria:**
- [ ] "Clear" button with confirmation dialog
- [ ] Confirmation prevents accidental data loss
- [ ] Option to save before clearing
- [ ] Clearing resets all transcript state
- [ ] Audio settings and server connection are preserved

**Scenario:**
```gherkin
Given I have an unsaved transcript
When I click the "Clear" button
Then I should see a confirmation dialog
And the dialog should warn about unsaved changes
And I should have options: "Save & Clear", "Clear Without Saving", "Cancel"

Given I confirm "Clear Without Saving"
When the action completes
Then the transcript panel should be empty
And the recording state should be reset
And my server connection should remain active
```

---

#### User Story 3.4: Save Transcript to History
**As a** user  
**I want to** save my transcript for later reference  
**So that** I can access it across sessions

**Acceptance Criteria:**
- [ ] "Save" button available after recording
- [ ] Custom title input with auto-generated default
- [ ] Saved transcripts appear in history list
- [ ] Save confirmation with link to view in history
- [ ] Auto-save option for long recordings

**Scenario:**
```gherkin
Given I have a completed transcript
When I click the "Save" button
Then I should see a dialog to enter a title
And the title field should have a default value (e.g., "Transcript - Dec 1, 2024")
And I should be able to edit the title

Given I enter a title and click "Save"
When the save completes
Then I should see a success notification
And the notification should include a "View in History" link
And the transcript should appear in my history list
```

---

#### User Story 3.5: View Transcript History
**As a** user  
**I want to** view my saved transcripts  
**So that** I can access and manage past recordings

**Acceptance Criteria:**
- [ ] History page lists all saved transcripts
- [ ] List shows title, date, duration, and word count
- [ ] Search functionality to find specific transcripts
- [ ] Sort options (date, title, duration)
- [ ] Pagination for large history lists
- [ ] Preview snippet for each transcript

**Scenario:**
```gherkin
Given I have saved multiple transcripts
When I navigate to the History page
Then I should see a list of my saved transcripts
And each item should show: title, date, duration, word count, preview
And I should see search and sort controls

Given I search for "meeting"
When I type in the search box
Then the list should filter to show only matching transcripts
And matches should be highlighted in the preview text
```

---

#### User Story 3.6: View Individual Transcript
**As a** user  
**I want to** view a saved transcript in detail  
**So that** I can read, edit, or export it

**Acceptance Criteria:**
- [ ] Full transcript text displayed with timestamps
- [ ] Edit title functionality
- [ ] Export options available
- [ ] Delete option with confirmation
- [ ] Back navigation to history list
- [ ] Keyboard navigation between segments

**Scenario:**
```gherkin
Given I am on the History page
When I click on a transcript item
Then I should navigate to the transcript detail page
And I should see the full transcript text
And I should see the title, date, and duration
And I should see Export and Delete buttons

Given I click the "Edit Title" button
When I enter a new title and save
Then the title should update
And I should see a success notification
```

---

#### User Story 3.7: Delete Transcript
**As a** user  
**I want to** delete transcripts I no longer need  
**So that** I can manage my storage and privacy

**Acceptance Criteria:**
- [ ] Delete button on transcript detail and history list
- [ ] Confirmation dialog prevents accidental deletion
- [ ] Bulk delete option for multiple transcripts
- [ ] Deleted transcripts are permanently removed
- [ ] Success feedback after deletion

**Scenario:**
```gherkin
Given I am viewing a transcript
When I click the "Delete" button
Then I should see a confirmation dialog
And the dialog should show the transcript title
And I should have "Delete" and "Cancel" options

Given I confirm deletion
When the action completes
Then I should be redirected to the History page
And the transcript should no longer appear in the list
And I should see a "Transcript deleted" notification
```

---

### Epic 4: Audio Configuration

#### User Story 4.1: Select Microphone Device
**As a** user  
**I want to** choose which microphone to use  
**So that** I can use my preferred audio input device

**Acceptance Criteria:**
- [ ] Dropdown list of available microphones
- [ ] Current selection is clearly indicated
- [ ] Device names are human-readable
- [ ] Refresh button to detect new devices
- [ ] Selection persists across sessions
- [ ] Preview audio level for selected device

**Scenario:**
```gherkin
Given I have multiple microphones connected
When I open the device selector
Then I should see a list of all available microphones
And the currently selected device should be highlighted
And each device should show its name

Given I select a different microphone
When the selection changes
Then the audio input should switch to the new device
And the audio meter should reflect the new device's input
And my selection should be saved for next time
```

---

#### User Story 4.2: Monitor Audio Levels
**As a** user  
**I want to** see my audio input levels  
**So that** I can ensure my microphone is working and properly positioned

**Acceptance Criteria:**
- [ ] Real-time audio level meter (RMS and Peak)
- [ ] Visual indication of clipping/too loud
- [ ] Visual indication of too quiet
- [ ] Meter updates at least 20 times per second
- [ ] Meter is visible during recording

**Scenario:**
```gherkin
Given I have granted microphone permission
When I view the audio controls panel
Then I should see an audio level meter
And the meter should respond to my voice in real-time
And the meter should show both RMS and Peak levels

Given my audio is too loud (clipping)
When the level exceeds the threshold
Then the meter should show a red/warning indicator
And I should see a "Too loud" warning message

Given my audio is too quiet
When the level is below the threshold
Then I should see a "Speak louder" suggestion
```

---

#### User Story 4.3: View Voice Activity Detection Status
**As a** user  
**I want to** see when the system detects my voice  
**So that** I know when my speech is being processed

**Acceptance Criteria:**
- [ ] VAD indicator shows speech/silence state
- [ ] Visual distinction between speaking and not speaking
- [ ] Indicator updates in real-time
- [ ] Accessible to screen readers

**Scenario:**
```gherkin
Given I am recording
When I am speaking
Then the VAD indicator should show "Speaking" state (e.g., green, active icon)
And the indicator should update within 100ms of voice detection

Given I stop speaking
When silence is detected
Then the VAD indicator should show "Silent" state (e.g., gray, inactive icon)
```

---

### Epic 5: Connection Management

#### User Story 5.1: View Connection Status
**As a** user  
**I want to** see the current connection status  
**So that** I know if the application is ready to transcribe

**Acceptance Criteria:**
- [ ] Status indicator always visible (header/status bar)
- [ ] Clear states: Disconnected, Connecting, Connected, Reconnecting, Error
- [ ] Color coding for quick recognition
- [ ] Tooltip with additional details on hover
- [ ] Status changes trigger notifications for important events

**Scenario:**
```gherkin
Given I open the application
When the page loads
Then I should see a connection status indicator
And the status should show "Connecting..." initially
And when connection succeeds, it should show "Connected" with green indicator

Given the connection is lost
When the WebSocket disconnects
Then the status should change to "Reconnecting..."
And I should see the reconnection attempt count
And a notification should inform me of the connection issue
```

---

#### User Story 5.2: Configure Server Connection
**As a** user  
**I want to** configure the STT server connection  
**So that** I can connect to my preferred server

**Acceptance Criteria:**
- [ ] Settings dialog accessible from header
- [ ] Server URL input with validation
- [ ] API key input (password field)
- [ ] Auth ID input (optional)
- [ ] Test connection button
- [ ] Save and apply settings

**Scenario:**
```gherkin
Given I want to change the server
When I click the Settings icon in the header
Then I should see a settings dialog
And I should see fields for: Server URL, API Key, Auth ID
And the current values should be pre-populated

Given I enter a new server URL
When I click "Test Connection"
Then I should see a loading indicator
And if successful, I should see "Connection successful" with latency
And if failed, I should see a specific error message

Given I click "Save"
When the settings are saved
Then the application should reconnect to the new server
And my settings should persist across sessions
```

---

#### User Story 5.3: Handle Connection Errors
**As a** user  
**I want to** understand and recover from connection errors  
**So that** I can continue using the application

**Acceptance Criteria:**
- [ ] Clear error messages for different failure types
- [ ] Suggested actions for each error type
- [ ] Manual reconnect button
- [ ] Auto-reconnect with exponential backoff
- [ ] Error details available for technical users

**Scenario:**
```gherkin
Given the server is unreachable
When connection fails
Then I should see "Unable to connect to server"
And I should see suggestions: "Check your internet connection" or "Verify server URL"
And I should see a "Retry" button

Given authentication fails (401)
When the server rejects my API key
Then I should see "Authentication failed"
And I should see a suggestion to check my API key
And I should see a link to open Settings

Given the connection drops during recording
When the WebSocket closes unexpectedly
Then recording should pause automatically
And I should see "Connection lost - Reconnecting..."
And the application should attempt to reconnect
And when reconnected, I should be prompted to resume recording
```

---

### Epic 6: Accessibility & Usability

#### User Story 6.1: Keyboard Navigation
**As a** user who prefers keyboard navigation  
**I want to** control the application using only my keyboard  
**So that** I can use it efficiently without a mouse

**Acceptance Criteria:**
- [ ] All interactive elements are focusable
- [ ] Logical tab order throughout the application
- [ ] Keyboard shortcuts for common actions
- [ ] Focus indicators are clearly visible
- [ ] Escape key closes modals and dropdowns
- [ ] Shortcut help dialog available

**Scenario:**
```gherkin
Given I am using keyboard navigation
When I press Tab
Then focus should move through interactive elements in logical order
And each focused element should have a visible focus indicator

Given I want to see available shortcuts
When I press "?" or click "Keyboard Shortcuts"
Then I should see a dialog listing all shortcuts:
  - Ctrl+Space: Toggle recording
  - Ctrl+P: Pause/Resume
  - Ctrl+S: Save transcript
  - Ctrl+C: Copy transcript (when focused)
  - Escape: Close dialogs
```

---

#### User Story 6.2: Screen Reader Support
**As a** user who uses a screen reader  
**I want to** have all content and actions announced properly  
**So that** I can use the application effectively

**Acceptance Criteria:**
- [ ] All images have alt text
- [ ] Form fields have associated labels
- [ ] Status changes are announced via ARIA live regions
- [ ] Buttons have descriptive accessible names
- [ ] Transcript content is readable by screen readers
- [ ] Dynamic content updates are announced

**Scenario:**
```gherkin
Given I am using a screen reader
When new transcript text appears
Then the screen reader should announce "New transcription: [text]"
And the announcement should not interrupt current reading

Given I click the Record button
When recording starts
Then the screen reader should announce "Recording started"
And the button's accessible name should change to "Stop recording"

Given the connection status changes
When the status updates
Then the screen reader should announce the new status
```

---

#### User Story 6.3: Theme Support
**As a** user  
**I want to** choose between light and dark themes  
**So that** I can use the application comfortably in different lighting conditions

**Acceptance Criteria:**
- [ ] Light, Dark, and System theme options
- [ ] Theme toggle easily accessible
- [ ] Theme preference persists across sessions
- [ ] System theme follows OS preference
- [ ] Smooth transition between themes
- [ ] All UI elements properly themed

**Scenario:**
```gherkin
Given I prefer dark mode
When I click the theme toggle
Then I should see options: Light, Dark, System
And selecting "Dark" should immediately apply dark theme
And my preference should be saved

Given I select "System" theme
When my OS is in dark mode
Then the application should use dark theme
And when I change my OS to light mode
Then the application should switch to light theme automatically
```

---

#### User Story 6.4: Responsive Design
**As a** user on a mobile device  
**I want to** use the application on my phone or tablet  
**So that** I can transcribe on the go

**Acceptance Criteria:**
- [ ] Layout adapts to screen size
- [ ] Touch-friendly button sizes (minimum 44x44px)
- [ ] Mobile navigation menu
- [ ] Transcript panel is scrollable and readable
- [ ] Audio controls remain accessible
- [ ] No horizontal scrolling required

**Scenario:**
```gherkin
Given I am using a mobile device (< 768px width)
When I view the application
Then the layout should be single-column
And the navigation should collapse to a hamburger menu
And buttons should be large enough to tap easily
And the transcript panel should fill the available width

Given I am using a tablet (768px - 1024px width)
When I view the application
Then the layout should show sidebar and main content
And touch interactions should work smoothly
```

---

### Epic 7: Advanced Features

#### User Story 7.1: Search Within Transcript
**As a** user  
**I want to** search for specific words in my transcript  
**So that** I can quickly find relevant sections

**Acceptance Criteria:**
- [ ] Search input field in transcript panel
- [ ] Real-time filtering as I type
- [ ] Highlight matching text
- [ ] Show match count
- [ ] Navigate between matches (next/previous)
- [ ] Keyboard shortcut (Ctrl+F)

**Scenario:**
```gherkin
Given I have a long transcript
When I click the search icon or press Ctrl+F
Then a search input should appear
And I should be able to type my search query

Given I search for "meeting"
When matches are found
Then all instances should be highlighted
And I should see "X of Y matches"
And I should be able to press Enter or click arrows to navigate between matches
```

---

#### User Story 7.2: Edit Transcript Text
**As a** user  
**I want to** edit transcription errors  
**So that** I can correct mistakes before exporting

**Acceptance Criteria:**
- [ ] Click/tap on text to edit
- [ ] Inline editing with save/cancel
- [ ] Edited segments are marked as modified
- [ ] Undo/redo support
- [ ] Original text can be restored

**Scenario:**
```gherkin
Given I see a transcription error
When I click on the text segment
Then the segment should become editable
And I should see Save and Cancel buttons

Given I edit the text and click Save
When the edit is saved
Then the text should update
And the segment should show an "edited" indicator
And I should be able to undo the change
```

---

#### User Story 7.3: Timestamp Navigation
**As a** user  
**I want to** click on timestamps to navigate  
**So that** I can quickly jump to specific parts of the transcript

**Acceptance Criteria:**
- [ ] Timestamps are clickable
- [ ] Clicking scrolls to that segment
- [ ] Visual highlight on the target segment
- [ ] Works in both live and saved transcripts

**Scenario:**
```gherkin
Given I have a long transcript with timestamps
When I click on a timestamp
Then the transcript should scroll to that segment
And the segment should be briefly highlighted
And the segment should be centered in the viewport if possible
```

---

#### User Story 7.4: Multi-Language Support (Future)
**As a** user who speaks multiple languages  
**I want to** transcribe in different languages  
**So that** I can use the application for various content

**Acceptance Criteria:**
- [ ] Language selector in settings
- [ ] Server-supported languages are listed
- [ ] Language preference persists
- [ ] UI language can be set independently

**Scenario:**
```gherkin
Given the server supports multiple languages
When I open the language settings
Then I should see a list of available languages
And I should be able to select my preferred language
And transcription should use the selected language
```

---

### Epic 8: Developer & Power User Features

#### User Story 8.1: View Debug Information
**As a** developer  
**I want to** see detailed debug information  
**So that** I can diagnose issues and understand system behavior

**Acceptance Criteria:**
- [ ] Debug panel accessible via settings or keyboard shortcut
- [ ] Shows WebSocket message log
- [ ] Shows audio pipeline statistics
- [ ] Shows connection details and latency
- [ ] Can be enabled/disabled
- [ ] Exportable debug log

**Scenario:**
```gherkin
Given I am a developer debugging an issue
When I enable debug mode (Settings > Developer > Enable Debug)
Then I should see a debug panel
And the panel should show:
  - WebSocket messages (sent/received)
  - Audio buffer statistics
  - Connection latency
  - Error logs

Given I want to share debug information
When I click "Export Debug Log"
Then a JSON file should download with all debug data
```

---

#### User Story 8.2: Custom Audio Settings
**As a** power user  
**I want to** configure advanced audio settings  
**So that** I can optimize for my specific use case

**Acceptance Criteria:**
- [ ] Sample rate selection (if server supports multiple)
- [ ] Buffer size configuration
- [ ] Noise suppression toggle (if available)
- [ ] Gain/volume adjustment
- [ ] Settings are validated before applying

**Scenario:**
```gherkin
Given I want to optimize audio quality
When I open Advanced Audio Settings
Then I should see options for:
  - Sample Rate (e.g., 16kHz, 24kHz, 48kHz)
  - Buffer Size (e.g., 1024, 2048, 4096 samples)
  - Noise Suppression (on/off)

Given I change the sample rate
When I save the settings
Then the audio pipeline should reconfigure
And I should see a notification if the server doesn't support the setting
```

---

#### User Story 8.3: API Key Management
**As a** user with multiple API keys  
**I want to** manage my saved API keys  
**So that** I can quickly switch between different servers/accounts

**Acceptance Criteria:**
- [ ] Save multiple server configurations
- [ ] Name/label each configuration
- [ ] Quick switch between saved configs
- [ ] Secure storage of API keys
- [ ] Delete saved configurations

**Scenario:**
```gherkin
Given I use multiple STT servers
When I open Server Configurations
Then I should see a list of saved configurations
And I should be able to add a new configuration
And each configuration should have: Name, URL, API Key, Auth ID

Given I want to switch servers
When I select a different configuration
Then the application should connect to the new server
And my current transcript should be preserved
```

---

### User Story Acceptance Checklist

For each user story to be considered complete:

- [ ] **Functionality**: All acceptance criteria are met
- [ ] **Testing**: Unit tests and integration tests pass
- [ ] **Accessibility**: WCAG 2.1 AA compliance verified
- [ ] **Responsiveness**: Works on mobile, tablet, and desktop
- [ ] **Performance**: Meets performance targets (< 100ms audio latency, < 500ms transcript latency)
- [ ] **Error Handling**: Graceful degradation and helpful error messages
- [ ] **Documentation**: User-facing features are documented
- [ ] **Code Review**: PR approved by at least one reviewer

---

### User Journey Maps

#### Journey 1: First-Time User - Quick Transcription

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        First-Time User Journey                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. DISCOVER          2. UNDERSTAND         3. PERMIT            4. RECORD  │
│  ┌─────────┐          ┌─────────┐          ┌─────────┐          ┌─────────┐ │
│  │ Land on │          │ Read    │          │ Grant   │          │ Click   │ │
│  │ homepage│───────▶  │ features│───────▶  │ mic     │───────▶  │ Record  │ │
│  │         │          │         │          │ access  │          │         │ │
│  └─────────┘          └─────────┘          └─────────┘          └─────────┘ │
│       │                    │                    │                    │       │
│       ▼                    ▼                    ▼                    ▼       │
│  "What is this?"     "Looks useful!"     "I trust this"      "It works!"   │
│                                                                              │
│  5. TRANSCRIBE        6. REVIEW           7. EXPORT           8. RETURN    │
│  ┌─────────┐          ┌─────────┐          ┌─────────┐          ┌─────────┐ │
│  │ Speak & │          │ Read    │          │ Download │          │ Save &  │ │
│  │ see text│───────▶  │ results │───────▶  │ as TXT  │───────▶  │ bookmark│ │
│  │         │          │         │          │         │          │         │ │
│  └─────────┘          └─────────┘          └─────────┘          └─────────┘ │
│       │                    │                    │                    │       │
│       ▼                    ▼                    ▼                    ▼       │
│  "Real-time!"        "Pretty accurate"   "Easy to use"      "I'll be back" │
│                                                                              │
│  Total Time: ~3 minutes from landing to first export                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Journey 2: Power User - Daily Workflow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Power User Daily Journey                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Morning Session                                                             │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐   │
│  │ Open    │    │ Check   │    │ Start   │    │ Pause   │    │ Resume  │   │
│  │ app     │───▶│ status  │───▶│ Ctrl+   │───▶│ for     │───▶│ Ctrl+   │   │
│  │         │    │ (green) │    │ Space   │    │ break   │    │ Space   │   │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘   │
│                                                                              │
│  End of Session                                                              │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐                   │
│  │ Stop    │    │ Quick   │    │ Save to │    │ Export  │                   │
│  │ Ctrl+   │───▶│ review  │───▶│ history │───▶│ as JSON │                   │
│  │ Space   │    │         │    │         │    │         │                   │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘                   │
│                                                                              │
│  Key Efficiency Features Used:                                               │
│  • Keyboard shortcuts for all actions                                        │
│  • Auto-save during long sessions                                            │
│  • Quick export without dialogs                                              │
│  • Persistent settings across sessions                                       │
│                                                                              │
│  Total Active Time: ~2 hours with multiple pause/resume cycles              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Journey 3: Accessibility User - Lecture Transcription

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Accessibility User Journey                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Before Lecture                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                      │
│  │ Open app    │    │ Verify      │    │ Set large   │                      │
│  │ with screen │───▶│ connection  │───▶│ text size   │                      │
│  │ reader      │    │ (announced) │    │ & contrast  │                      │
│  └─────────────┘    └─────────────┘    └─────────────┘                      │
│                                                                              │
│  During Lecture                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                      │
│  │ Tab to      │    │ Read live   │    │ Search for  │                      │
│  │ Record,     │───▶│ captions    │───▶│ key terms   │                      │
│  │ press Enter │    │ on screen   │    │ Ctrl+F      │                      │
│  └─────────────┘    └─────────────┘    └─────────────┘                      │
│                                                                              │
│  After Lecture                                                               │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                      │
│  │ Stop with   │    │ Navigate    │    │ Export for  │                      │
│  │ Ctrl+Space  │───▶│ transcript  │───▶│ study notes │                      │
│  │             │    │ with arrows │    │             │                      │
│  └─────────────┘    └─────────────┘    └─────────────┘                      │
│                                                                              │
│  Accessibility Features Used:                                                │
│  • Full keyboard navigation                                                  │
│  • Screen reader announcements for all status changes                        │
│  • High contrast theme                                                       │
│  • Large, readable text                                                      │
│  • ARIA live regions for real-time updates                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Browser Client                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────┐    ┌─────────────┐ │
│  │ Microphone  │───▶│ AudioContext │───▶│ AudioWorklet│───▶│  WebSocket  │ │
│  │   Input     │    │   (Web API)  │    │  Processor  │    │   Client    │ │
│  └─────────────┘    └──────────────┘    └─────────────┘    └──────┬──────┘ │
│                                                                    │        │
│  ┌─────────────────────────────────────────────────────────────────┼──────┐ │
│  │                         React Application                       │      │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │      │ │
│  │  │ Transcript   │  │   Audio      │  │  Connection  │◀─────────┘      │ │
│  │  │   Store      │  │   Store      │  │    Store     │                 │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                 │ │
│  │         │                 │                 │                          │ │
│  │         ▼                 ▼                 ▼                          │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │ │
│  │  │                    UI Components (shadcn/ui)                    │  │ │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │  │ │
│  │  │  │Transcript│  │  Audio  │  │ Status  │  │Settings │            │  │ │
│  │  │  │  Panel  │  │  Meter  │  │  Bar    │  │  Modal  │            │  │ │
│  │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │  │ │
│  │  └─────────────────────────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      │ WebSocket (ws:// or wss://)
                                      │ Binary: MessagePack audio chunks
                                      │ Text: JSON transcription messages
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Kyutai Rust STT Server (moshi-server)                     │
│                         stt.fullen.dev (via Caddy)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│  Endpoints:                                                                  │
│  • GET  /api/asr-streaming        - WebSocket streaming ASR                  │
│  • POST /api/asr-streaming        - Batch ASR (audio file upload)            │
│  • GET  /api/asr-streaming/health - Health check (200 OK)                    │
│  • GET  /api/build_info           - Server version info                      │
│  • GET  /api/modules_info         - Loaded modules info                      │
├─────────────────────────────────────────────────────────────────────────────┤
│  Authentication (auth.rs):                                                   │
│  • kyutai-api-key header OR auth_id query param (legacy API key)             │
│  • Authorization: Bearer <jwt> (Better Auth JWT)                             │
│  • better-auth.session_token cookie (session cookie)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────┐    ┌─────────────┐ │
│  │  WebSocket  │───▶│    Mimi      │───▶│ BatchedAsr  │───▶│ Transcript  │ │
│  │   Handler   │    │   Decoder    │    │   Engine    │    │   Sender    │ │
│  └─────────────┘    └──────────────┘    └─────────────┘    └─────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Audio Capture**: Browser captures microphone input via `getUserMedia()`
2. **Audio Processing**: AudioWorklet processes raw PCM to f32 samples at 24kHz
3. **Encoding**: Audio chunks encoded as MessagePack binary frames
4. **Transmission**: WebSocket sends binary audio, receives JSON transcripts
5. **Display**: React components render real-time transcription updates

---

## Technical Requirements

### Browser Requirements

| Feature | Minimum Version |
|---------|-----------------|
| Chrome | 66+ (AudioWorklet) |
| Firefox | 76+ (AudioWorklet) |
| Safari | 14.1+ (AudioWorklet) |
| Edge | 79+ (Chromium-based) |

### Server Requirements

#### moshi-server (STT Backend)
- Kyutai Rust STT server running on accessible endpoint
- Default deployment: `stt.fullen.dev` (via Caddy reverse proxy)
- WebSocket endpoint: `/api/asr-streaming`
- Health check endpoint: `/api/asr-streaming/health`
- Support for `ws://` (development) and `wss://` (production)
- Authentication: API key or Better Auth JWT

#### Better Auth Server (Optional)
- Node.js auth server for user authentication
- Located at: `moshi/auth-server/`
- Default port: 3001
- Required environment: `BETTER_AUTH_SECRET`, `DATABASE_URL`
- Provides: Email/password auth, session management, JWT tokens

### Technology Stack

| Layer | Technology |
|-------|------------|
| **Framework** | Next.js 15 (App Router) |
| **Language** | TypeScript 5.x |
| **Styling** | Tailwind CSS 3.x |
| **Components** | shadcn/ui |
| **Icons** | Lucide React |
| **State** | Zustand |
| **Authentication** | Better Auth (JWT cookie cache) |
| **Audio** | Web Audio API + AudioWorklet |
| **Audio Encoding** | opus-recorder (Opus codec via WASM) |
| **Audio Decoding** | decoderWorker (Opus WASM) |
| **WebSocket** | Native WebSocket API |
| **Protocol** | Custom binary protocol (see `protocol/encoder.ts`) |
| **Testing** | Vitest + Playwright |
| **Linting** | Biome |
| **Bundler** | Rspack |
| **Package Manager** | pnpm |

### Project Initialization

The Next.js web client will reside in its own repository (e.g., `kyutai-web-client`). Navigate to your new repository folder and initialize the project:

```bash
pnpm create next-app@latest ./ \
  --typescript \
  --react-compiler \
  --tailwind \
  --biome \
  --app \
  --no-src-dir \
  --rspack \
  --import-alias "@/*" \
  --use-pnpm \
  --yes
```

**Flags Explained:**

| Flag | Description |
|------|-------------|
| `./` | Initialize in current directory |
| `--typescript` | Enable TypeScript support |
| `--react-compiler` | Enable React Compiler (experimental) |
| `--tailwind` | Include Tailwind CSS configuration |
| `--biome` | Use Biome for linting/formatting (instead of ESLint) |
| `--app` | Use App Router (not Pages Router) |
| `--no-src-dir` | Place app directory at root (not in `src/`) |
| `--rspack` | Use Rspack bundler (faster than Webpack) |
| `--import-alias "@/*"` | Configure `@/` import alias for cleaner imports |
| `--use-pnpm` | Use pnpm as package manager |
| `--yes` | Skip all prompts, use defaults |

### Post-Initialization Setup

After project initialization, install additional dependencies.

**Note on Audio Encoding/Decoding:**
The moshi client uses **opus-recorder** for audio encoding and a pre-built **decoderWorker** for decoding. These handle Opus codec operations via WebAssembly.

**Required Assets:**
Copy the following files from `moshi/client/public/assets/` to your `public/assets/` directory:
- `decoderWorker.min.js` - Opus decoder worker script
- `decoderWorker.min.wasm` - Opus decoder WASM binary

**Audio Protocol:**
The moshi server uses a custom binary protocol (not MessagePack). See `moshi/client/src/protocol/encoder.ts` for the message format:
- `0x00` - Handshake
- `0x01` - Audio data (Opus-encoded)
- `0x02` - Text
- `0x03` - Control messages
- `0x04` - Metadata
- `0x05` - Error
- `0x06` - Ping

**Install other dependencies:**

```bash
# shadcn/ui CLI and components
pnpm dlx shadcn@latest init

# Core dependencies
pnpm add zustand nanoid

# Audio encoding (Opus codec via WASM)
pnpm add opus-recorder

# Video duration fix (for WebM recordings)
pnpm add webm-duration-fix

# Authentication (Better Auth)
pnpm add better-auth

# React Query for API state management
pnpm add @tanstack/react-query

# Form validation
pnpm add zod

# Environment boundary protection (Server/Client Component safety)
pnpm add server-only client-only

# Theme support (light/dark mode)
pnpm add next-themes

# Development dependencies
pnpm add -D vitest @vitejs/plugin-react jsdom @testing-library/react
pnpm add -D playwright @playwright/test
```

### shadcn/ui Component Installation

Install all required shadcn/ui components:

```bash
pnpm dlx shadcn@latest add \
  button card dialog input label select switch separator \
  dropdown-menu tooltip badge alert scroll-area progress \
  skeleton sheet tabs sonner alert-dialog
```

---

## Frontend Architecture

### Directory Structure

Components are annotated with `[S]` for Server Components and `[C]` for Client Components.

```
app/
├── layout.tsx                 # [S] Root layout - wraps Providers
├── page.tsx                   # [S] Main page - composes components
├── globals.css                # Global styles + Tailwind
├── (routes)/
│   ├── settings/
│   │   └── page.tsx           # [S] Settings page shell
│   └── history/
│       ├── page.tsx           # [S] Fetches transcript list
│       └── [id]/
│           └── page.tsx       # [S] Fetches single transcript
├── api/
│   └── health/
│       └── route.ts           # API route (server-only)

components/
├── ui/                        # [C] shadcn/ui components (all client)
│   ├── button.tsx
│   ├── card.tsx
│   ├── dialog.tsx
│   ├── input.tsx
│   ├── select.tsx
│   ├── slider.tsx
│   ├── switch.tsx
│   ├── toast.tsx
│   └── tooltip.tsx
├── audio/
│   ├── audio-controls.tsx     # [C] useState, onClick, AudioContext
│   ├── audio-meter.tsx        # [C] useEffect, requestAnimationFrame
│   ├── device-selector.tsx    # [C] navigator.mediaDevices
│   └── vad-indicator.tsx      # [C] Subscribes to Zustand store
├── transcript/
│   ├── transcript-panel.tsx   # [C] useRef, useEffect (auto-scroll)
│   ├── transcript-line.tsx    # [C] memo, hover interactions
│   ├── transcript-skeleton.tsx # [S] Static loading skeleton
│   ├── partial-text.tsx       # [C] CSS animation
│   └── final-text.tsx         # [S] Static text display
├── connection/
│   ├── connection-status.tsx  # [C] Subscribes to connection store
│   ├── server-config.tsx      # [C] Form state, localStorage
│   └── reconnect-button.tsx   # [C] onClick handler
├── layout/
│   ├── header.tsx             # [C] Mobile menu state, theme toggle
│   ├── footer.tsx             # [S] Static footer content
│   ├── nav-links.tsx          # [S] Static navigation links
│   └── sidebar.tsx            # [C] Collapsible state
└── providers/
    ├── providers.tsx          # [C] Combined providers wrapper
    ├── audio-provider.tsx     # [C] Audio context provider
    ├── websocket-provider.tsx # [C] WebSocket connection
    ├── query-provider.tsx     # [C] React Query provider
    └── theme-provider.tsx     # [C] next-themes provider

hooks/                         # All hooks are client-only
├── use-audio-capture.ts       # [C] Microphone capture hook
├── use-audio-worklet.ts       # [C] AudioWorklet management
├── use-websocket.ts           # [C] WebSocket connection hook
├── use-transcript.ts          # [C] Transcript state hook
├── use-audio-devices.ts       # [C] Device enumeration hook
├── use-local-storage.ts       # [C] Persistent settings hook
└── use-auth.ts                # [C] Better Auth authentication hook

lib/
├── api/
│   ├── client.ts              # [C] Client-side API calls
│   ├── server.ts              # [S] Server-only API (import 'server-only')
│   └── hooks.ts               # [C] React Query hooks
├── auth/
│   ├── auth-client.ts         # [C] Better Auth client configuration
│   └── types.ts               # Shared auth types (both)
├── audio/
│   ├── audio-context.ts       # [C] import 'client-only'
│   ├── audio-processor.ts     # [C] AudioWorklet for playback
│   └── constants.ts           # Shared constants (both)
├── decoder/
│   └── decoderWorker.ts       # [C] Opus decoder worker loader
├── protocol/
│   ├── encoder.ts             # [C] Binary message encoding/decoding
│   └── types.ts               # Shared protocol types (both)
├── websocket/
│   ├── client.ts              # [C] WebSocket client class
│   ├── security.ts            # [C] URL validation, auth helpers
│   └── types.ts               # Shared types (both)
├── stores/                    # All Zustand stores are client-only
│   ├── audio-store.ts         # [C] Audio state
│   ├── transcript-store.ts    # [C] Transcript state
│   └── connection-store.ts    # [C] Connection state
├── env.ts                     # [S] Server-only env validation
└── utils/
    ├── format-time.ts         # Shared (both)
    ├── debounce.ts            # [C] Client utility
    └── cn.ts                  # Shared (both)

public/
├── assets/
│   ├── decoderWorker.min.js   # Opus decoder worker (from moshi/client)
│   ├── decoderWorker.min.wasm # Opus decoder WASM binary
│   └── images/                # App images
├── worklets/
│   └── audio-processor.js     # AudioWorklet processor script (playback)
└── icons/
    └── ...                    # App icons

types/                         # Shared types (both environments)
├── audio.ts                   # Audio-related types
├── transcript.ts              # Transcript message types
├── websocket.ts               # WebSocket message types
└── settings.ts                # Settings types
```

### Server and Client Components

Following Next.js App Router best practices, components are categorized based on their requirements:

#### Component Classification

| Component Type | Rendering | Use When |
|----------------|-----------|----------|
| **Server Component** | Server-side (default) | Static UI, data fetching, no interactivity |
| **Client Component** | Client-side (`'use client'`) | State, effects, event handlers, browser APIs |

#### Server Components (No `'use client'` directive)

These components render on the server and send HTML to the client with zero JavaScript:

```
app/
├── layout.tsx                 # Server - wraps providers, static shell
├── page.tsx                   # Server - composes client components
├── (routes)/
│   ├── settings/
│   │   └── page.tsx           # Server - settings page shell
│   └── history/
│       ├── page.tsx           # Server - fetches transcript list
│       └── [id]/
│           └── page.tsx       # Server - fetches single transcript

components/
├── layout/
│   ├── footer.tsx             # Server - static footer content
│   └── nav-links.tsx          # Server - static navigation links
├── transcript/
│   └── transcript-skeleton.tsx # Server - loading skeleton
└── icons/
    └── logo.tsx               # Server - static SVG logo
```

**Example: Server Component Page**

```tsx
// app/page.tsx (Server Component - NO 'use client')
import { Suspense } from 'react';
import { Header } from '@/components/layout/header';
import { Footer } from '@/components/layout/footer';
import { TranscriptPanel } from '@/components/transcript/transcript-panel';
import { AudioControls } from '@/components/audio/audio-controls';
import { AudioMeter } from '@/components/audio/audio-meter';
import { DeviceSelector } from '@/components/audio/device-selector';
import { TranscriptSkeleton } from '@/components/transcript/transcript-skeleton';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Settings2 } from 'lucide-react';

// This is a Server Component - it orchestrates Client Components
export default function HomePage() {
  return (
    <div className="min-h-screen bg-background flex flex-col">
      {/* Header is a Client Component for interactivity */}
      <Header />
      
      <main className="container py-6 flex-1">
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
          <aside className="lg:col-span-1 space-y-4">
            {/* Client Components for audio interactivity */}
            <AudioControls />
            <AudioMeter />
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="flex items-center gap-2 text-base">
                  <Settings2 className="h-4 w-4" />
                  Audio Settings
                </CardTitle>
              </CardHeader>
              <CardContent>
                <DeviceSelector />
              </CardContent>
            </Card>
          </aside>
          
          <div className="lg:col-span-3">
            {/* Suspense boundary for streaming */}
            <Suspense fallback={<TranscriptSkeleton />}>
              <TranscriptPanel />
            </Suspense>
          </div>
        </div>
      </main>
      
      {/* Footer is a Server Component - static content */}
      <Footer />
    </div>
  );
}
```

**Example: Server Component with Data Fetching**

```tsx
// app/(routes)/history/page.tsx (Server Component)
import { TranscriptList } from '@/components/transcript/transcript-list';
import { api } from '@/lib/api/server'; // Server-only API client

// Server Component can fetch data directly
export default async function HistoryPage() {
  // This runs on the server - can use secrets, direct DB access
  const transcripts = await api.getTranscripts();
  
  return (
    <div className="container py-6">
      <h1 className="text-2xl font-bold mb-6">Transcript History</h1>
      {/* Pass server data to Client Component as props */}
      <TranscriptList initialData={transcripts} />
    </div>
  );
}
```

#### Client Components (Require `'use client'` directive)

These components need the `'use client'` directive because they use:
- React hooks (`useState`, `useEffect`, `useRef`, etc.)
- Event handlers (`onClick`, `onChange`, etc.)
- Browser APIs (`navigator`, `localStorage`, `AudioContext`, etc.)
- Third-party libraries that use client features

```
components/
├── audio/
│   ├── audio-controls.tsx     # Client - useState, onClick, AudioContext
│   ├── audio-meter.tsx        # Client - useEffect, requestAnimationFrame
│   ├── device-selector.tsx    # Client - navigator.mediaDevices
│   └── vad-indicator.tsx      # Client - subscribes to store
├── transcript/
│   ├── transcript-panel.tsx   # Client - useRef, useEffect (auto-scroll)
│   ├── transcript-line.tsx    # Client - memo, tooltips with hover
│   └── partial-text.tsx       # Client - animation
├── connection/
│   ├── connection-status.tsx  # Client - subscribes to connection store
│   ├── server-config.tsx      # Client - form state, localStorage
│   └── reconnect-button.tsx   # Client - onClick handler
├── layout/
│   ├── header.tsx             # Client - mobile menu state, theme toggle
│   └── theme-toggle.tsx       # Client - useTheme hook
└── providers/
    ├── audio-provider.tsx     # Client - context provider
    ├── websocket-provider.tsx # Client - WebSocket connection
    ├── query-provider.tsx     # Client - React Query provider
    └── theme-provider.tsx     # Client - next-themes provider
```

#### Provider Pattern for Context

Context providers must be Client Components, but should wrap Server Component children:

```tsx
// components/providers/providers.tsx
'use client';

import { ThemeProvider } from 'next-themes';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { TooltipProvider } from '@/components/ui/tooltip';
import { Toaster } from '@/components/ui/sonner';
import { useState } from 'react';

interface ProvidersProps {
  children: React.ReactNode;
}

export function Providers({ children }: ProvidersProps) {
  // Create QueryClient inside component to avoid sharing between requests
  const [queryClient] = useState(() => new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 60 * 1000,
        refetchOnWindowFocus: false,
      },
    },
  }));

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider
        attribute="class"
        defaultTheme="system"
        enableSystem
        disableTransitionOnChange
      >
        <TooltipProvider delayDuration={300}>
          {children}
          <Toaster position="bottom-right" />
        </TooltipProvider>
      </ThemeProvider>
    </QueryClientProvider>
  );
}
```

```tsx
// app/layout.tsx (Server Component)
import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import { Providers } from '@/components/providers/providers';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Kyutai STT - Real-time Speech-to-Text',
  description: 'Stream your microphone to a speech-to-text server.',
};

// Root layout is a Server Component
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={inter.className}>
        {/* Providers is a Client Component, children are Server Components */}
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
```

#### Interleaving Pattern: Server Components as Children

Pass Server Components as `children` to Client Components to keep them server-rendered:

```tsx
// components/layout/collapsible-section.tsx
'use client';

import { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

interface CollapsibleSectionProps {
  title: string;
  children: React.ReactNode; // Can be Server Components!
  defaultOpen?: boolean;
}

export function CollapsibleSection({ 
  title, 
  children, 
  defaultOpen = true 
}: CollapsibleSectionProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className="border rounded-lg">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-between p-4"
      >
        <span className="font-medium">{title}</span>
        <ChevronDown className={cn(
          "h-4 w-4 transition-transform",
          isOpen && "rotate-180"
        )} />
      </button>
      {isOpen && (
        <div className="p-4 pt-0">
          {/* children can be Server Components - they're already rendered */}
          {children}
        </div>
      )}
    </div>
  );
}
```

```tsx
// app/page.tsx (Server Component)
import { CollapsibleSection } from '@/components/layout/collapsible-section';
import { ServerRenderedStats } from '@/components/stats/server-stats';

export default function Page() {
  return (
    <CollapsibleSection title="Statistics">
      {/* This Server Component is passed as children to Client Component */}
      <ServerRenderedStats />
    </CollapsibleSection>
  );
}
```

#### Environment Boundary Protection

Use `server-only` and `client-only` packages to prevent accidental cross-environment imports:

```typescript
// lib/api/server.ts
import 'server-only'; // Will error if imported in Client Component

import { env } from '@/lib/env';

// Safe to use server secrets here
export async function getTranscriptsFromDB() {
  const response = await fetch(env.DATABASE_URL, {
    headers: {
      Authorization: `Bearer ${env.DATABASE_SECRET}`, // Server-only secret
    },
  });
  return response.json();
}
```

```typescript
// lib/audio/audio-context.ts
import 'client-only'; // Will error if imported in Server Component

// Safe to use browser APIs here
export function createAudioContext(): AudioContext {
  return new (window.AudioContext || (window as any).webkitAudioContext)();
}
```

#### Component Decision Tree

```
┌─────────────────────────────────────────────────────────────────┐
│                    Does this component need...                   │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│ useState,     │    │ Browser APIs  │    │ Event         │
│ useEffect,    │    │ (window,      │    │ handlers      │
│ useRef, etc.  │    │ localStorage, │    │ (onClick,     │
│               │    │ AudioContext) │    │ onChange)     │
└───────┬───────┘    └───────┬───────┘    └───────┬───────┘
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   YES to any?   │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
     ┌────────────────┐           ┌────────────────┐
     │      YES       │           │       NO       │
     │                │           │                │
     │ 'use client'   │           │ Server         │
     │ Client         │           │ Component      │
     │ Component      │           │ (default)      │
     └────────────────┘           └────────────────┘
```

#### Best Practices Summary

1. **Start with Server Components** - They're the default; only add `'use client'` when needed
2. **Push `'use client'` down** - Keep it at the leaf components, not at the top
3. **Use composition** - Pass Server Components as `children` to Client Components
4. **Colocate providers** - Create a single `Providers` wrapper for all context providers
5. **Protect boundaries** - Use `server-only` and `client-only` packages
6. **Minimize client bundle** - Only the components with `'use client'` add to JS bundle
7. **Fetch data in Server Components** - Pass data as props to Client Components
8. **Use Suspense** - Wrap async Server Components for streaming

### State Management (Zustand)

#### Audio Store

```typescript
// lib/stores/audio-store.ts
interface AudioState {
  // State
  isRecording: boolean;
  isPaused: boolean;
  selectedDeviceId: string | null;
  availableDevices: MediaDeviceInfo[];
  audioLevel: number;        // 0-1 normalized RMS
  peakLevel: number;         // 0-1 normalized peak
  sampleRate: number;
  
  // Actions
  startRecording: () => Promise<void>;
  stopRecording: () => void;
  pauseRecording: () => void;
  resumeRecording: () => void;
  selectDevice: (deviceId: string) => void;
  updateAudioLevel: (rms: number, peak: number) => void;
  refreshDevices: () => Promise<void>;
}
```

#### Transcript Store

```typescript
// lib/stores/transcript-store.ts
interface TranscriptWord {
  word: string;
  confidence: number;
  startTime: number;
  endTime: number;
}

interface TranscriptSegment {
  id: string;
  type: 'partial' | 'final';
  text: string;
  confidence: number;
  timestamp: number;
  words?: TranscriptWord[];
  utteranceId?: string;
}

interface TranscriptState {
  // State
  segments: TranscriptSegment[];
  currentPartial: TranscriptSegment | null;
  isProcessing: boolean;
  
  // Actions
  addPartial: (segment: TranscriptSegment) => void;
  addFinal: (segment: TranscriptSegment) => void;
  clearTranscript: () => void;
  exportTranscript: () => string;
}
```

#### Connection Store

```typescript
// lib/stores/connection-store.ts
type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';

interface ConnectionState {
  // State
  status: ConnectionStatus;
  serverUrl: string;
  apiKey: string | null;
  authId: string | null;
  error: string | null;
  reconnectAttempt: number;
  maxReconnectAttempts: number;
  
  // Actions
  connect: () => Promise<void>;
  disconnect: () => void;
  setServerUrl: (url: string) => void;
  setApiKey: (key: string | null) => void;
  setAuthId: (id: string | null) => void;
  setError: (error: string | null) => void;
}
```

---

## Backend Architecture

### Overview

The Next.js backend serves multiple purposes:
1. **Static frontend hosting** with SSR/SSG capabilities
2. **API routes** for health checks, configuration, and transcript persistence
3. **Optional WebSocket proxy** for restricted network environments
4. **Server-side validation** and security middleware

### Directory Structure

```
app/
├── api/
│   ├── health/
│   │   └── route.ts              # Health check endpoint
│   ├── config/
│   │   └── route.ts              # Server configuration endpoint
│   ├── transcripts/
│   │   ├── route.ts              # List/create transcripts
│   │   └── [id]/
│   │       └── route.ts          # Get/update/delete transcript
│   ├── ws-proxy/
│   │   └── route.ts              # WebSocket proxy (optional)
│   └── validate/
│       └── route.ts              # URL/key validation
├── (main)/
│   ├── page.tsx                  # Home page
│   ├── settings/
│   │   └── page.tsx              # Settings page
│   └── history/
│       └── page.tsx              # Transcript history page
└── layout.tsx                    # Root layout
```

---

### API Route Specifications

#### 1. Health Check Endpoint

**File:** `app/api/health/route.ts`  
**Method:** GET  
**Purpose:** Kubernetes/Docker health probes, monitoring

```typescript
// app/api/health/route.ts
import { NextResponse } from 'next/server';

interface HealthResponse {
  status: 'ok' | 'degraded' | 'error';
  timestamp: string;
  version: string;
  uptime: number;
  checks: {
    database?: 'ok' | 'error';
    memory: 'ok' | 'warning' | 'error';
  };
}

const startTime = Date.now();

export async function GET(): Promise<NextResponse<HealthResponse>> {
  const memoryUsage = process.memoryUsage();
  const heapUsedMB = memoryUsage.heapUsed / 1024 / 1024;
  
  const memoryStatus = 
    heapUsedMB > 450 ? 'error' :
    heapUsedMB > 350 ? 'warning' : 'ok';
  
  const response: HealthResponse = {
    status: memoryStatus === 'error' ? 'degraded' : 'ok',
    timestamp: new Date().toISOString(),
    version: process.env.npm_package_version || '0.0.0',
    uptime: Math.floor((Date.now() - startTime) / 1000),
    checks: {
      memory: memoryStatus,
    },
  };
  
  return NextResponse.json(response, {
    status: response.status === 'ok' ? 200 : 503,
  });
}
```

#### 2. Configuration Endpoint

**File:** `app/api/config/route.ts`  
**Method:** GET  
**Purpose:** Provide client-side configuration from server environment

```typescript
// app/api/config/route.ts
import { NextResponse } from 'next/server';

interface ClientConfig {
  defaultServerUrl: string;
  defaultSampleRate: number;
  defaultBlockSize: number;
  maxReconnectAttempts: number;
  reconnectDelayMs: number;
  features: {
    historyEnabled: boolean;
    exportEnabled: boolean;
    vadIndicatorEnabled: boolean;
  };
}

export async function GET(): Promise<NextResponse<ClientConfig>> {
  const config: ClientConfig = {
    defaultServerUrl: process.env.DEFAULT_STT_URL || 'ws://localhost:8080/api/asr-streaming',
    defaultSampleRate: parseInt(process.env.DEFAULT_SAMPLE_RATE || '24000', 10),
    defaultBlockSize: parseInt(process.env.DEFAULT_BLOCK_SIZE || '1920', 10),
    maxReconnectAttempts: parseInt(process.env.MAX_RECONNECT_ATTEMPTS || '5', 10),
    reconnectDelayMs: parseInt(process.env.RECONNECT_DELAY_MS || '1500', 10),
    features: {
      historyEnabled: process.env.FEATURE_HISTORY === 'true',
      exportEnabled: process.env.FEATURE_EXPORT !== 'false',
      vadIndicatorEnabled: process.env.FEATURE_VAD !== 'false',
    },
  };
  
  return NextResponse.json(config, {
    headers: {
      'Cache-Control': 'public, max-age=300, stale-while-revalidate=60',
    },
  });
}
```

#### 3. Server URL Validation Endpoint

**File:** `app/api/validate/route.ts`  
**Method:** POST  
**Purpose:** Validate WebSocket URL and optionally test connectivity

```typescript
// app/api/validate/route.ts
import { NextRequest, NextResponse } from 'next/server';

interface ValidateRequest {
  url: string;
  testConnection?: boolean;
}

interface ValidateResponse {
  valid: boolean;
  errors: string[];
  warnings: string[];
  serverInfo?: {
    reachable: boolean;
    latencyMs?: number;
    serverVersion?: string;
  };
}

export async function POST(request: NextRequest): Promise<NextResponse<ValidateResponse>> {
  try {
    const body: ValidateRequest = await request.json();
    const { url, testConnection } = body;
    
    const errors: string[] = [];
    const warnings: string[] = [];
    
    // Validate URL format
    let parsedUrl: URL;
    try {
      parsedUrl = new URL(url);
    } catch {
      return NextResponse.json({
        valid: false,
        errors: ['Invalid URL format'],
        warnings: [],
      });
    }
    
    // Check protocol
    if (!['ws:', 'wss:'].includes(parsedUrl.protocol)) {
      errors.push('URL must use ws:// or wss:// protocol');
    }
    
    // Warn about insecure connections in production
    if (parsedUrl.protocol === 'ws:' && process.env.NODE_ENV === 'production') {
      warnings.push('Using insecure ws:// in production is not recommended');
    }
    
    // Check for localhost in production
    if (
      process.env.NODE_ENV === 'production' &&
      ['localhost', '127.0.0.1', '::1'].includes(parsedUrl.hostname)
    ) {
      warnings.push('Localhost URLs may not work in production');
    }
    
    // Check path
    if (!parsedUrl.pathname.includes('asr') && !parsedUrl.pathname.includes('streaming')) {
      warnings.push('URL path does not appear to be an ASR streaming endpoint');
    }
    
    const response: ValidateResponse = {
      valid: errors.length === 0,
      errors,
      warnings,
    };
    
    // Optional connectivity test
    if (testConnection && errors.length === 0) {
      try {
        const httpUrl = url.replace('ws://', 'http://').replace('wss://', 'https://');
        const healthUrl = new URL('/health', httpUrl).toString();
        
        const startTime = Date.now();
        const healthResponse = await fetch(healthUrl, {
          method: 'GET',
          signal: AbortSignal.timeout(5000),
        });
        const latencyMs = Date.now() - startTime;
        
        response.serverInfo = {
          reachable: healthResponse.ok,
          latencyMs,
        };
        
        if (healthResponse.ok) {
          const healthData = await healthResponse.json();
          response.serverInfo.serverVersion = healthData.version;
        }
      } catch (error) {
        response.serverInfo = {
          reachable: false,
        };
        warnings.push('Could not reach server health endpoint');
      }
    }
    
    return NextResponse.json(response);
  } catch (error) {
    return NextResponse.json(
      {
        valid: false,
        errors: ['Invalid request body'],
        warnings: [],
      },
      { status: 400 }
    );
  }
}
```

#### 4. Transcript Persistence API

**File:** `app/api/transcripts/route.ts`  
**Methods:** GET, POST  
**Purpose:** List and create transcript records

```typescript
// app/api/transcripts/route.ts
import { NextRequest, NextResponse } from 'next/server';
import { nanoid } from 'nanoid';

// Types
interface TranscriptSegment {
  id: string;
  type: 'partial' | 'final';
  text: string;
  confidence: number;
  timestamp: number;
  words?: {
    word: string;
    confidence: number;
    startTime: number;
    endTime: number;
  }[];
}

interface Transcript {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  duration: number;
  segmentCount: number;
  wordCount: number;
  segments: TranscriptSegment[];
  metadata: {
    serverUrl: string;
    sampleRate: number;
    userAgent?: string;
  };
}

interface TranscriptListItem {
  id: string;
  title: string;
  createdAt: string;
  duration: number;
  wordCount: number;
  preview: string;
}

// In-memory storage (replace with database in production)
const transcripts = new Map<string, Transcript>();

// GET /api/transcripts - List all transcripts
export async function GET(request: NextRequest): Promise<NextResponse> {
  const searchParams = request.nextUrl.searchParams;
  const page = parseInt(searchParams.get('page') || '1', 10);
  const limit = parseInt(searchParams.get('limit') || '20', 10);
  const search = searchParams.get('search') || '';
  const sortBy = searchParams.get('sortBy') || 'createdAt';
  const sortOrder = searchParams.get('sortOrder') || 'desc';
  
  let items = Array.from(transcripts.values());
  
  // Search filter
  if (search) {
    const searchLower = search.toLowerCase();
    items = items.filter(t => 
      t.title.toLowerCase().includes(searchLower) ||
      t.segments.some(s => s.text.toLowerCase().includes(searchLower))
    );
  }
  
  // Sort
  items.sort((a, b) => {
    const aVal = a[sortBy as keyof Transcript];
    const bVal = b[sortBy as keyof Transcript];
    const comparison = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
    return sortOrder === 'desc' ? -comparison : comparison;
  });
  
  // Paginate
  const total = items.length;
  const startIndex = (page - 1) * limit;
  const paginatedItems = items.slice(startIndex, startIndex + limit);
  
  // Map to list items (exclude full segments)
  const listItems: TranscriptListItem[] = paginatedItems.map(t => ({
    id: t.id,
    title: t.title,
    createdAt: t.createdAt,
    duration: t.duration,
    wordCount: t.wordCount,
    preview: t.segments
      .filter(s => s.type === 'final')
      .slice(0, 3)
      .map(s => s.text)
      .join(' ')
      .slice(0, 150) + '...',
  }));
  
  return NextResponse.json({
    items: listItems,
    pagination: {
      page,
      limit,
      total,
      totalPages: Math.ceil(total / limit),
    },
  });
}

// POST /api/transcripts - Create new transcript
export async function POST(request: NextRequest): Promise<NextResponse> {
  try {
    const body = await request.json();
    
    const { title, segments, metadata } = body;
    
    if (!segments || !Array.isArray(segments)) {
      return NextResponse.json(
        { error: 'segments array is required' },
        { status: 400 }
      );
    }
    
    const id = nanoid();
    const now = new Date().toISOString();
    
    // Calculate stats
    const finalSegments = segments.filter((s: TranscriptSegment) => s.type === 'final');
    const wordCount = finalSegments.reduce((acc: number, s: TranscriptSegment) => 
      acc + s.text.split(/\s+/).filter(Boolean).length, 0
    );
    
    const timestamps = segments.map((s: TranscriptSegment) => s.timestamp);
    const duration = timestamps.length > 0 
      ? (Math.max(...timestamps) - Math.min(...timestamps)) / 1000 
      : 0;
    
    const transcript: Transcript = {
      id,
      title: title || `Transcript ${new Date().toLocaleDateString()}`,
      createdAt: now,
      updatedAt: now,
      duration,
      segmentCount: segments.length,
      wordCount,
      segments,
      metadata: metadata || {},
    };
    
    transcripts.set(id, transcript);
    
    return NextResponse.json(transcript, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      { error: 'Invalid request body' },
      { status: 400 }
    );
  }
}
```

**File:** `app/api/transcripts/[id]/route.ts`  
**Methods:** GET, PUT, DELETE  
**Purpose:** Individual transcript operations

```typescript
// app/api/transcripts/[id]/route.ts
import { NextRequest, NextResponse } from 'next/server';

// Reference the same transcripts Map from parent route
// In production, use a proper database

interface RouteParams {
  params: { id: string };
}

// GET /api/transcripts/[id] - Get single transcript
export async function GET(
  request: NextRequest,
  { params }: RouteParams
): Promise<NextResponse> {
  const { id } = params;
  
  const transcript = transcripts.get(id);
  
  if (!transcript) {
    return NextResponse.json(
      { error: 'Transcript not found' },
      { status: 404 }
    );
  }
  
  return NextResponse.json(transcript);
}

// PUT /api/transcripts/[id] - Update transcript
export async function PUT(
  request: NextRequest,
  { params }: RouteParams
): Promise<NextResponse> {
  const { id } = params;
  
  const transcript = transcripts.get(id);
  
  if (!transcript) {
    return NextResponse.json(
      { error: 'Transcript not found' },
      { status: 404 }
    );
  }
  
  try {
    const body = await request.json();
    const { title } = body;
    
    if (title) {
      transcript.title = title;
    }
    
    transcript.updatedAt = new Date().toISOString();
    transcripts.set(id, transcript);
    
    return NextResponse.json(transcript);
  } catch (error) {
    return NextResponse.json(
      { error: 'Invalid request body' },
      { status: 400 }
    );
  }
}

// DELETE /api/transcripts/[id] - Delete transcript
export async function DELETE(
  request: NextRequest,
  { params }: RouteParams
): Promise<NextResponse> {
  const { id } = params;
  
  if (!transcripts.has(id)) {
    return NextResponse.json(
      { error: 'Transcript not found' },
      { status: 404 }
    );
  }
  
  transcripts.delete(id);
  
  return new NextResponse(null, { status: 204 });
}
```

#### 5. Transcript Export Endpoint

**File:** `app/api/transcripts/[id]/export/route.ts`  
**Method:** GET  
**Purpose:** Export transcript in various formats

```typescript
// app/api/transcripts/[id]/export/route.ts
import { NextRequest, NextResponse } from 'next/server';

type ExportFormat = 'txt' | 'json' | 'srt' | 'vtt';

interface RouteParams {
  params: { id: string };
}

export async function GET(
  request: NextRequest,
  { params }: RouteParams
): Promise<NextResponse> {
  const { id } = params;
  const format = (request.nextUrl.searchParams.get('format') || 'txt') as ExportFormat;
  
  const transcript = transcripts.get(id);
  
  if (!transcript) {
    return NextResponse.json(
      { error: 'Transcript not found' },
      { status: 404 }
    );
  }
  
  const finalSegments = transcript.segments.filter(s => s.type === 'final');
  
  switch (format) {
    case 'txt':
      return new NextResponse(
        finalSegments.map(s => s.text).join('\n\n'),
        {
          headers: {
            'Content-Type': 'text/plain; charset=utf-8',
            'Content-Disposition': `attachment; filename="${transcript.title}.txt"`,
          },
        }
      );
    
    case 'json':
      return NextResponse.json(transcript, {
        headers: {
          'Content-Disposition': `attachment; filename="${transcript.title}.json"`,
        },
      });
    
    case 'srt':
      const srtContent = finalSegments.map((s, i) => {
        const startTime = formatSrtTime(s.timestamp);
        const endTime = formatSrtTime(s.timestamp + 3000); // Assume 3s duration
        return `${i + 1}\n${startTime} --> ${endTime}\n${s.text}\n`;
      }).join('\n');
      
      return new NextResponse(srtContent, {
        headers: {
          'Content-Type': 'text/plain; charset=utf-8',
          'Content-Disposition': `attachment; filename="${transcript.title}.srt"`,
        },
      });
    
    case 'vtt':
      const vttContent = 'WEBVTT\n\n' + finalSegments.map((s, i) => {
        const startTime = formatVttTime(s.timestamp);
        const endTime = formatVttTime(s.timestamp + 3000);
        return `${startTime} --> ${endTime}\n${s.text}\n`;
      }).join('\n');
      
      return new NextResponse(vttContent, {
        headers: {
          'Content-Type': 'text/vtt; charset=utf-8',
          'Content-Disposition': `attachment; filename="${transcript.title}.vtt"`,
        },
      });
    
    default:
      return NextResponse.json(
        { error: 'Unsupported format' },
        { status: 400 }
      );
  }
}

function formatSrtTime(ms: number): string {
  const hours = Math.floor(ms / 3600000);
  const minutes = Math.floor((ms % 3600000) / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const milliseconds = ms % 1000;
  
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')},${milliseconds.toString().padStart(3, '0')}`;
}

function formatVttTime(ms: number): string {
  const hours = Math.floor(ms / 3600000);
  const minutes = Math.floor((ms % 3600000) / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const milliseconds = ms % 1000;
  
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}.${milliseconds.toString().padStart(3, '0')}`;
}
```

---

### WebSocket Proxy (Optional)

For environments where direct WebSocket connections to the STT server are blocked (corporate firewalls, etc.), an optional WebSocket proxy can be implemented.

**File:** `app/api/ws-proxy/route.ts`

```typescript
// app/api/ws-proxy/route.ts
// Note: Next.js App Router doesn't natively support WebSocket upgrades.
// This requires a custom server or Edge Runtime with WebSocket support.

import { NextRequest } from 'next/server';

export const runtime = 'edge';

export async function GET(request: NextRequest) {
  const upgradeHeader = request.headers.get('upgrade');
  
  if (upgradeHeader !== 'websocket') {
    return new Response('Expected WebSocket upgrade', { status: 426 });
  }
  
  const targetUrl = request.nextUrl.searchParams.get('target');
  const apiKey = request.headers.get('x-api-key');
  
  if (!targetUrl) {
    return new Response('Missing target URL', { status: 400 });
  }
  
  // Validate target URL
  try {
    const url = new URL(targetUrl);
    if (!['ws:', 'wss:'].includes(url.protocol)) {
      return new Response('Invalid target protocol', { status: 400 });
    }
  } catch {
    return new Response('Invalid target URL', { status: 400 });
  }
  
  // Note: Actual WebSocket proxying requires platform-specific implementation
  // For Vercel Edge Functions, use the WebSocket API
  // For Node.js custom server, use 'ws' or 'http-proxy'
  
  return new Response('WebSocket proxy not implemented', { status: 501 });
}
```

**Alternative: Custom Server WebSocket Proxy**

```typescript
// server/ws-proxy.ts
// For use with a custom Next.js server

import { WebSocketServer, WebSocket } from 'ws';
import { createServer } from 'http';
import next from 'next';

const dev = process.env.NODE_ENV !== 'production';
const app = next({ dev });
const handle = app.getRequestHandler();

app.prepare().then(() => {
  const server = createServer((req, res) => {
    handle(req, res);
  });
  
  const wss = new WebSocketServer({ 
    server,
    path: '/api/ws-proxy',
  });
  
  wss.on('connection', (clientWs, req) => {
    const url = new URL(req.url || '', `http://${req.headers.host}`);
    const targetUrl = url.searchParams.get('target');
    const apiKey = req.headers['x-api-key'] as string | undefined;
    
    if (!targetUrl) {
      clientWs.close(4000, 'Missing target URL');
      return;
    }
    
    // Connect to target STT server
    const targetWs = new WebSocket(targetUrl, {
      headers: apiKey ? { 'kyutai-api-key': apiKey } : {},
    });
    
    // Proxy messages bidirectionally
    clientWs.on('message', (data) => {
      if (targetWs.readyState === WebSocket.OPEN) {
        targetWs.send(data);
      }
    });
    
    targetWs.on('message', (data) => {
      if (clientWs.readyState === WebSocket.OPEN) {
        clientWs.send(data);
      }
    });
    
    // Handle close events
    clientWs.on('close', () => {
      targetWs.close();
    });
    
    targetWs.on('close', () => {
      clientWs.close();
    });
    
    // Handle errors
    clientWs.on('error', (err) => {
      console.error('Client WebSocket error:', err);
      targetWs.close();
    });
    
    targetWs.on('error', (err) => {
      console.error('Target WebSocket error:', err);
      clientWs.close(4001, 'Target connection error');
    });
  });
  
  const port = parseInt(process.env.PORT || '3000', 10);
  server.listen(port, () => {
    console.log(`> Ready on http://localhost:${port}`);
  });
});
```

---

### Server Actions

Next.js Server Actions for form submissions and mutations.

**File:** `lib/actions/transcript-actions.ts`

```typescript
// lib/actions/transcript-actions.ts
'use server';

import { revalidatePath } from 'next/cache';
import { redirect } from 'next/navigation';

interface SaveTranscriptInput {
  title: string;
  segments: Array<{
    id: string;
    type: 'partial' | 'final';
    text: string;
    confidence: number;
    timestamp: number;
  }>;
  metadata: {
    serverUrl: string;
    sampleRate: number;
  };
}

export async function saveTranscript(input: SaveTranscriptInput) {
  const response = await fetch(`${process.env.NEXT_PUBLIC_APP_URL}/api/transcripts`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(input),
  });
  
  if (!response.ok) {
    throw new Error('Failed to save transcript');
  }
  
  const transcript = await response.json();
  
  revalidatePath('/history');
  
  return transcript;
}

export async function deleteTranscript(id: string) {
  const response = await fetch(`${process.env.NEXT_PUBLIC_APP_URL}/api/transcripts/${id}`, {
    method: 'DELETE',
  });
  
  if (!response.ok) {
    throw new Error('Failed to delete transcript');
  }
  
  revalidatePath('/history');
}

export async function renameTranscript(id: string, title: string) {
  const response = await fetch(`${process.env.NEXT_PUBLIC_APP_URL}/api/transcripts/${id}`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ title }),
  });
  
  if (!response.ok) {
    throw new Error('Failed to rename transcript');
  }
  
  revalidatePath('/history');
  revalidatePath(`/history/${id}`);
  
  return response.json();
}
```

---

### Middleware

**File:** `middleware.ts`

```typescript
// middleware.ts
import { NextResponse } from 'next/server';
import type { NextRequest } from 'next/server';

export function middleware(request: NextRequest) {
  const response = NextResponse.next();
  
  // Add security headers
  response.headers.set('X-Content-Type-Options', 'nosniff');
  response.headers.set('X-Frame-Options', 'DENY');
  response.headers.set('X-XSS-Protection', '1; mode=block');
  response.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
  
  // CORS for API routes
  if (request.nextUrl.pathname.startsWith('/api/')) {
    const origin = request.headers.get('origin');
    const allowedOrigins = process.env.ALLOWED_ORIGINS?.split(',') || [];
    
    if (origin && (allowedOrigins.includes(origin) || allowedOrigins.includes('*'))) {
      response.headers.set('Access-Control-Allow-Origin', origin);
      response.headers.set('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
      response.headers.set('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-API-Key');
    }
    
    // Handle preflight
    if (request.method === 'OPTIONS') {
      return new NextResponse(null, { status: 204, headers: response.headers });
    }
  }
  
  // Rate limiting header (actual limiting done by reverse proxy/Vercel)
  response.headers.set('X-RateLimit-Limit', '100');
  
  return response;
}

export const config = {
  matcher: [
    '/((?!_next/static|_next/image|favicon.ico).*)',
  ],
};
```

---

### Database Schema (Optional - for Production)

For production deployments with persistent storage, use Prisma with PostgreSQL or SQLite.

**File:** `prisma/schema.prisma`

```prisma
// prisma/schema.prisma
generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model Transcript {
  id           String   @id @default(cuid())
  title        String
  createdAt    DateTime @default(now())
  updatedAt    DateTime @updatedAt
  duration     Float    @default(0)
  segmentCount Int      @default(0)
  wordCount    Int      @default(0)
  
  // Metadata
  serverUrl    String?
  sampleRate   Int?
  userAgent    String?
  
  // Relations
  segments     TranscriptSegment[]
  
  @@index([createdAt])
  @@index([title])
}

model TranscriptSegment {
  id           String     @id @default(cuid())
  transcriptId String
  type         String     // 'partial' | 'final'
  text         String
  confidence   Float
  timestamp    BigInt
  orderIndex   Int
  
  // Relations
  transcript   Transcript @relation(fields: [transcriptId], references: [id], onDelete: Cascade)
  words        Word[]
  
  @@index([transcriptId])
  @@index([timestamp])
}

model Word {
  id        String            @id @default(cuid())
  segmentId String
  word      String
  confidence Float
  startTime Float
  endTime   Float
  orderIndex Int
  
  // Relations
  segment   TranscriptSegment @relation(fields: [segmentId], references: [id], onDelete: Cascade)
  
  @@index([segmentId])
}
```

---

### Environment Configuration

**File:** `.env.example`

```env
# Application
NODE_ENV=development
NEXT_PUBLIC_APP_URL=http://localhost:3000

# STT Server Defaults
DEFAULT_STT_URL=wss://stt.fullen.dev/api/asr-streaming
DEFAULT_SAMPLE_RATE=24000
DEFAULT_BLOCK_SIZE=1920
MAX_RECONNECT_ATTEMPTS=5
RECONNECT_DELAY_MS=1500

# Authentication (Better Auth)
# URL of the Better Auth server (defaults to same origin if not set)
NEXT_PUBLIC_AUTH_URL=
# Shared secret for JWT validation (must match moshi-server's BETTER_AUTH_SECRET)
# Note: This is only needed if running auth server separately
BETTER_AUTH_SECRET=your-32-character-secret-here

# Feature Flags
FEATURE_HISTORY=true
FEATURE_EXPORT=true
FEATURE_VAD=true
FEATURE_WS_PROXY=false

# Security
ALLOWED_ORIGINS=http://localhost:3000

# Database (optional, for production)
DATABASE_URL=postgresql://user:password@localhost:5432/stt_web_client

# Analytics (optional)
NEXT_PUBLIC_ANALYTICS_ID=
```

**File:** `lib/env.ts` - Type-safe environment access

```typescript
// lib/env.ts
import { z } from 'zod';

const envSchema = z.object({
  NODE_ENV: z.enum(['development', 'production', 'test']).default('development'),
  NEXT_PUBLIC_APP_URL: z.string().url().optional(),
  
  // STT Server
  DEFAULT_STT_URL: z.string().default('ws://localhost:8080/api/asr-streaming'),
  DEFAULT_SAMPLE_RATE: z.coerce.number().default(24000),
  DEFAULT_BLOCK_SIZE: z.coerce.number().default(1920),
  MAX_RECONNECT_ATTEMPTS: z.coerce.number().default(5),
  RECONNECT_DELAY_MS: z.coerce.number().default(1500),
  
  // Features
  FEATURE_HISTORY: z.coerce.boolean().default(true),
  FEATURE_EXPORT: z.coerce.boolean().default(true),
  FEATURE_VAD: z.coerce.boolean().default(true),
  FEATURE_WS_PROXY: z.coerce.boolean().default(false),
  
  // Security
  ALLOWED_ORIGINS: z.string().optional(),
  
  // Database
  DATABASE_URL: z.string().optional(),
});

export type Env = z.infer<typeof envSchema>;

function validateEnv(): Env {
  const parsed = envSchema.safeParse(process.env);
  
  if (!parsed.success) {
    console.error('❌ Invalid environment variables:', parsed.error.flatten().fieldErrors);
    throw new Error('Invalid environment variables');
  }
  
  return parsed.data;
}

export const env = validateEnv();
```

---

### API Client Library

**File:** `lib/api/client.ts`

```typescript
// lib/api/client.ts

interface ApiClientOptions {
  baseUrl?: string;
}

class ApiClient {
  private baseUrl: string;
  
  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = options.baseUrl || '';
  }
  
  private async request<T>(
    path: string,
    options: RequestInit = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    
    const response = await fetch(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
    });
    
    if (!response.ok) {
      const error = await response.json().catch(() => ({}));
      throw new ApiError(response.status, error.message || 'Request failed');
    }
    
    if (response.status === 204) {
      return undefined as T;
    }
    
    return response.json();
  }
  
  // Health
  async getHealth() {
    return this.request<{
      status: string;
      timestamp: string;
      version: string;
      uptime: number;
    }>('/api/health');
  }
  
  // Config
  async getConfig() {
    return this.request<{
      defaultServerUrl: string;
      defaultSampleRate: number;
      defaultBlockSize: number;
      maxReconnectAttempts: number;
      reconnectDelayMs: number;
      features: {
        historyEnabled: boolean;
        exportEnabled: boolean;
        vadIndicatorEnabled: boolean;
      };
    }>('/api/config');
  }
  
  // Validation
  async validateUrl(url: string, testConnection = false) {
    return this.request<{
      valid: boolean;
      errors: string[];
      warnings: string[];
      serverInfo?: {
        reachable: boolean;
        latencyMs?: number;
        serverVersion?: string;
      };
    }>('/api/validate', {
      method: 'POST',
      body: JSON.stringify({ url, testConnection }),
    });
  }
  
  // Transcripts
  async listTranscripts(params: {
    page?: number;
    limit?: number;
    search?: string;
    sortBy?: string;
    sortOrder?: 'asc' | 'desc';
  } = {}) {
    const searchParams = new URLSearchParams();
    if (params.page) searchParams.set('page', params.page.toString());
    if (params.limit) searchParams.set('limit', params.limit.toString());
    if (params.search) searchParams.set('search', params.search);
    if (params.sortBy) searchParams.set('sortBy', params.sortBy);
    if (params.sortOrder) searchParams.set('sortOrder', params.sortOrder);
    
    return this.request<{
      items: Array<{
        id: string;
        title: string;
        createdAt: string;
        duration: number;
        wordCount: number;
        preview: string;
      }>;
      pagination: {
        page: number;
        limit: number;
        total: number;
        totalPages: number;
      };
    }>(`/api/transcripts?${searchParams}`);
  }
  
  async getTranscript(id: string) {
    return this.request<{
      id: string;
      title: string;
      createdAt: string;
      updatedAt: string;
      duration: number;
      segmentCount: number;
      wordCount: number;
      segments: Array<{
        id: string;
        type: 'partial' | 'final';
        text: string;
        confidence: number;
        timestamp: number;
      }>;
      metadata: {
        serverUrl: string;
        sampleRate: number;
      };
    }>(`/api/transcripts/${id}`);
  }
  
  async createTranscript(data: {
    title: string;
    segments: Array<{
      id: string;
      type: 'partial' | 'final';
      text: string;
      confidence: number;
      timestamp: number;
    }>;
    metadata: {
      serverUrl: string;
      sampleRate: number;
    };
  }) {
    return this.request<{ id: string }>('/api/transcripts', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }
  
  async updateTranscript(id: string, data: { title: string }) {
    return this.request<{ id: string }>(`/api/transcripts/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }
  
  async deleteTranscript(id: string) {
    return this.request<void>(`/api/transcripts/${id}`, {
      method: 'DELETE',
    });
  }
  
  async exportTranscript(id: string, format: 'txt' | 'json' | 'srt' | 'vtt' = 'txt') {
    const response = await fetch(`${this.baseUrl}/api/transcripts/${id}/export?format=${format}`);
    
    if (!response.ok) {
      throw new ApiError(response.status, 'Export failed');
    }
    
    return response.blob();
  }
}

class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export const api = new ApiClient();
export { ApiError };
```

---

### React Query Integration

**File:** `lib/api/hooks.ts`

```typescript
// lib/api/hooks.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from './client';

// Query Keys
export const queryKeys = {
  health: ['health'] as const,
  config: ['config'] as const,
  transcripts: {
    all: ['transcripts'] as const,
    list: (params: Record<string, unknown>) => ['transcripts', 'list', params] as const,
    detail: (id: string) => ['transcripts', 'detail', id] as const,
  },
};

// Hooks
export function useHealth() {
  return useQuery({
    queryKey: queryKeys.health,
    queryFn: () => api.getHealth(),
    refetchInterval: 30000, // 30 seconds
  });
}

export function useConfig() {
  return useQuery({
    queryKey: queryKeys.config,
    queryFn: () => api.getConfig(),
    staleTime: 5 * 60 * 1000, // 5 minutes
  });
}

export function useTranscripts(params: {
  page?: number;
  limit?: number;
  search?: string;
  sortBy?: string;
  sortOrder?: 'asc' | 'desc';
} = {}) {
  return useQuery({
    queryKey: queryKeys.transcripts.list(params),
    queryFn: () => api.listTranscripts(params),
  });
}

export function useTranscript(id: string) {
  return useQuery({
    queryKey: queryKeys.transcripts.detail(id),
    queryFn: () => api.getTranscript(id),
    enabled: !!id,
  });
}

export function useCreateTranscript() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: api.createTranscript.bind(api),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.transcripts.all });
    },
  });
}

export function useUpdateTranscript() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: { title: string } }) =>
      api.updateTranscript(id, data),
    onSuccess: (_, { id }) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.transcripts.detail(id) });
      queryClient.invalidateQueries({ queryKey: queryKeys.transcripts.all });
    },
  });
}

export function useDeleteTranscript() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: api.deleteTranscript.bind(api),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.transcripts.all });
    },
  });
}

export function useValidateUrl() {
  return useMutation({
    mutationFn: ({ url, testConnection }: { url: string; testConnection?: boolean }) =>
      api.validateUrl(url, testConnection),
  });
}
```

---

### Backend GitHub Issues

#### Issue #36: API Health Endpoint
**Branch:** `feature/api-health`
**Labels:** `backend`, `api`, `priority:high`

**Tasks:**
- [ ] Create `/api/health` route
- [ ] Add memory usage check
- [ ] Add uptime tracking
- [ ] Return appropriate status codes

---

#### Issue #37: Configuration API
**Branch:** `feature/api-config`
**Labels:** `backend`, `api`, `priority:high`

**Tasks:**
- [ ] Create `/api/config` route
- [ ] Expose feature flags
- [ ] Add caching headers
- [ ] Create type-safe env validation

---

#### Issue #38: Transcript Persistence API
**Branch:** `feature/api-transcripts`
**Labels:** `backend`, `api`, `database`, `priority:medium`

**Tasks:**
- [ ] Create CRUD endpoints for transcripts
- [ ] Implement pagination and search
- [ ] Add export functionality (txt, json, srt, vtt)
- [ ] Set up Prisma schema (optional)

---

#### Issue #39: URL Validation API
**Branch:** `feature/api-validate`
**Labels:** `backend`, `api`, `priority:medium`

**Tasks:**
- [ ] Create `/api/validate` route
- [ ] Validate WebSocket URL format
- [ ] Optional connectivity test
- [ ] Return warnings for insecure configs

---

#### Issue #40: Middleware & Security
**Branch:** `feature/middleware`
**Labels:** `backend`, `security`, `priority:high`

**Tasks:**
- [ ] Create Next.js middleware
- [ ] Add security headers
- [ ] Implement CORS handling
- [ ] Add rate limiting headers

---

#### Issue #41: Server Actions
**Branch:** `feature/server-actions`
**Labels:** `backend`, `priority:medium`

**Tasks:**
- [ ] Create transcript server actions
- [ ] Implement cache revalidation
- [ ] Add error handling

---

#### Issue #42: API Client Library
**Branch:** `feature/api-client`
**Labels:** `backend`, `frontend`, `priority:medium`

**Tasks:**
- [ ] Create typed API client
- [ ] Add React Query hooks
- [ ] Implement error handling
- [ ] Add query key management

---

#### Issue #43: WebSocket Proxy (Optional)
**Branch:** `feature/ws-proxy`
**Labels:** `backend`, `websocket`, `priority:low`

**Tasks:**
- [ ] Implement Edge Runtime proxy stub
- [ ] Create custom server proxy option
- [ ] Document proxy configuration
- [ ] Add connection validation

---

## WebSocket Protocol Integration

### Message Types

#### Client → Server (Binary)

Audio chunks are sent as **Opus-encoded binary frames** using a custom binary protocol (not MessagePack).

See the [Binary Protocol Specification](#binary-protocol-specification) in the Existing Moshi Client Implementation Reference section for the complete protocol definition.

```typescript
// lib/protocol/encoder.ts
// Audio message: 0x01 prefix + Opus-encoded audio data
export function encodeAudioMessage(opusData: Uint8Array): Uint8Array {
  return new Uint8Array([0x01, ...opusData]);
}
```

#### Server → Client (Binary)

Server messages are also binary-encoded using the same protocol:

```typescript
// lib/websocket/types.ts
interface PartialTranscription {
  type: 'partial';
  text: string;
  confidence: number;
  timestamp: number;
  word_alternatives?: WordAlternative[];
  vad_markers?: VadMarker[];
}

interface FinalTranscription {
  type: 'final';
  text: string;
  confidence: number;
  timestamp: number;
  utterance_id: string;
  word_alternatives?: WordAlternative[];
  vad_markers?: VadMarker[];
}

interface ErrorMessage {
  type: 'error';
  error: string;
  message: string;
  timestamp: number;
  suggestion?: string;
}

interface StatusMessage {
  type: 'status';
  status: string;
  message: string;
  timestamp: number;
  server_info?: ServerInfo;
}

type ServerMessage = PartialTranscription | FinalTranscription | ErrorMessage | StatusMessage;
```

### WebSocket Client Implementation

```typescript
// lib/websocket/client.ts
import { buildAuthenticatedWsUrl } from './security';

export type ConnectionStatus = 
  | 'disconnected' 
  | 'connecting' 
  | 'connected' 
  | 'reconnecting' 
  | 'error';

export interface STTClientOptions {
  /** Legacy API key for authentication */
  apiKey?: string;
  /** Better Auth session token (JWT) */
  sessionToken?: string;
  /** Maximum reconnection attempts before giving up */
  maxReconnectAttempts?: number;
  /** Base delay between reconnection attempts (ms) */
  reconnectDelay?: number;
  /** Callback for incoming server messages */
  onMessage: (message: ServerMessage) => void;
  /** Callback for connection status changes */
  onStatusChange: (status: ConnectionStatus) => void;
  /** Callback for errors */
  onError: (error: Error) => void;
}

export class STTWebSocketClient {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private readonly maxReconnectAttempts: number;
  private readonly reconnectDelay: number;
  
  constructor(
    private readonly url: string,
    private readonly options: STTClientOptions
  ) {
    this.maxReconnectAttempts = options.maxReconnectAttempts ?? 5;
    this.reconnectDelay = options.reconnectDelay ?? 1500;
  }
  
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const wsUrl = this.buildUrl();
      this.options.onStatusChange('connecting');
      
      this.ws = new WebSocket(wsUrl);
      this.ws.binaryType = 'arraybuffer';
      
      this.ws.onopen = () => {
        this.reconnectAttempts = 0;
        this.options.onStatusChange('connected');
        resolve();
      };
      
      this.ws.onmessage = (event) => {
        if (typeof event.data === 'string') {
          try {
            const message = JSON.parse(event.data) as ServerMessage;
            this.options.onMessage(message);
          } catch (e) {
            console.error('Failed to parse server message:', e);
          }
        }
      };
      
      this.ws.onerror = (event) => {
        this.options.onError(new Error('WebSocket error'));
      };
      
      this.ws.onclose = (event) => {
        // Handle authentication failures (401 Unauthorized)
        if (event.code === 1008 || event.reason?.includes('Unauthorized')) {
          this.options.onStatusChange('error');
          this.options.onError(new Error('Authentication failed'));
          return;
        }
        
        if (!event.wasClean) {
          this.attemptReconnect();
        } else {
          this.options.onStatusChange('disconnected');
        }
      };
    });
  }
  
  sendAudio(chunk: Uint8Array): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(chunk);
    }
  }
  
  disconnect(): void {
    this.ws?.close(1000, 'Client disconnect');
    this.ws = null;
  }
  
  /** Update authentication token (e.g., after token refresh) */
  updateAuth(sessionToken: string): void {
    this.options.sessionToken = sessionToken;
  }
  
  private buildUrl(): string {
    return buildAuthenticatedWsUrl(this.url, {
      apiKey: this.options.apiKey,
      sessionToken: this.options.sessionToken,
    });
  }
  
  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      this.options.onStatusChange('error');
      this.options.onError(new Error('Max reconnection attempts reached'));
      return;
    }
    
    this.reconnectAttempts++;
    this.options.onStatusChange('reconnecting');
    
    const delay = this.reconnectDelay * this.reconnectAttempts;
    setTimeout(() => this.connect(), delay);
  }
}
```

---

## Audio Pipeline Design

The audio pipeline uses **opus-recorder** for Opus encoding and a pre-built **decoderWorker** for decoding. This matches the existing moshi client implementation in `moshi/client/`.

### Pipeline Overview

1.  **Audio Capture**: `getUserMedia` captures raw audio from microphone.
2.  **opus-recorder**: Encodes audio to Opus format at 24kHz, mono.
3.  **WebSocket**: Transmits Opus-encoded chunks using custom binary protocol.
4.  **decoderWorker**: Decodes incoming Opus audio from server.
5.  **AudioWorklet**: Plays back decoded audio with buffering.

### Key Components

| Component | Source | Purpose |
|-----------|--------|---------|
| `opus-recorder` | npm package | Opus encoding via WASM |
| `decoderWorker.min.js` | `public/assets/` | Opus decoding worker |
| `decoderWorker.min.wasm` | `public/assets/` | Opus decoder WASM binary |
| `audio-processor.ts` | `src/` | AudioWorklet for playback buffering |
| `protocol/encoder.ts` | `src/protocol/` | Binary message encoding/decoding |

### opus-recorder Configuration

The moshi client configures opus-recorder with specific settings for low-latency streaming:

```typescript
// From moshi/client/src/pages/Conversation/hooks/useUserAudio.ts
import Recorder from 'opus-recorder';
import encoderPath from 'opus-recorder/dist/encoderWorker.min.js?url';

const recorderOptions = {
  encoderPath,                    // Path to encoder worker
  bufferLength: 960,              // Buffer size (960 = 24000 / 12.5 / 2)
  encoderFrameSize: 20,           // 20ms frames
  encoderSampleRate: 24000,       // Target sample rate
  maxFramesPerPage: 2,            // Low latency: 2 frames per page
  numberOfChannels: 1,            // Mono audio
  recordingGain: 1,               // No gain adjustment
  resampleQuality: 3,             // Resampling quality (0-10)
  encoderComplexity: 0,           // Low complexity for speed
  encoderApplication: 2049,       // OPUS_APPLICATION_VOIP
  streamPages: true,              // Stream pages as they're ready
};

const recorder = new Recorder(recorderOptions);

recorder.ondataavailable = (data: Uint8Array) => {
  // Send Opus-encoded audio chunk via WebSocket
  sendMessage({ type: 'audio', data });
};
```

### Binary Protocol Encoder

The moshi server uses a custom binary protocol, not MessagePack:

```typescript
// lib/protocol/encoder.ts
// Based on moshi/client/src/protocol/encoder.ts

export type WSMessage =
  | { type: 'handshake'; version: number; model: number }
  | { type: 'audio'; data: Uint8Array }
  | { type: 'text'; data: string }
  | { type: 'control'; action: 'start' | 'endTurn' | 'pause' | 'restart' }
  | { type: 'metadata'; data: unknown }
  | { type: 'error'; data: string }
  | { type: 'ping' };

const CONTROL_MESSAGES_MAP = {
  start: 0b00000000,
  endTurn: 0b00000001,
  pause: 0b00000010,
  restart: 0b00000011,
} as const;

export function encodeMessage(message: WSMessage): Uint8Array {
  switch (message.type) {
    case 'handshake':
      return new Uint8Array([0x00, message.version, message.model]);
    case 'audio':
      return new Uint8Array([0x01, ...message.data]);
    case 'text':
      return new Uint8Array([0x02, ...new TextEncoder().encode(message.data)]);
    case 'control':
      return new Uint8Array([0x03, CONTROL_MESSAGES_MAP[message.action]]);
    case 'metadata':
      return new Uint8Array([0x04, ...new TextEncoder().encode(JSON.stringify(message.data))]);
    case 'error':
      return new Uint8Array([0x05, ...new TextEncoder().encode(message.data)]);
    case 'ping':
      return new Uint8Array([0x06]);
  }
}

export function decodeMessage(data: Uint8Array): WSMessage {
  const type = data[0];
  const payload = data.slice(1);
  
  switch (type) {
    case 0x00:
      return { type: 'handshake', version: 0, model: 0 };
    case 0x01:
      return { type: 'audio', data: payload };
    case 0x02:
      return { type: 'text', data: new TextDecoder().decode(payload) };
    case 0x03: {
      const actionMap = { 0: 'start', 1: 'endTurn', 2: 'pause', 3: 'restart' } as const;
      return { type: 'control', action: actionMap[payload[0] as keyof typeof actionMap] };
    }
    case 0x04:
      return { type: 'metadata', data: JSON.parse(new TextDecoder().decode(payload)) };
    case 0x05:
      return { type: 'error', data: new TextDecoder().decode(payload) };
    case 0x06:
      return { type: 'ping' };
    default:
      throw new Error(`Unknown message type: ${type}`);
  }
}
```

### AudioWorklet Processor (Playback)

The AudioWorklet handles buffered playback of decoded audio from the server:

```typescript
// Based on moshi/client/src/audio-processor.ts
// This is for PLAYBACK, not capture

class MoshiProcessor extends AudioWorkletProcessor {
  private frames: Float32Array[] = [];
  private offsetInFirstBuffer = 0;
  private started = false;
  private initialBufferSamples: number;
  
  constructor() {
    super();
    // Buffer for 80ms at sample rate
    const frameSize = Math.round(80 * sampleRate / 1000);
    this.initialBufferSamples = frameSize;
    
    this.port.onmessage = (event) => {
      if (event.data.type === 'reset') {
        this.frames = [];
        this.offsetInFirstBuffer = 0;
        this.started = false;
        return;
      }
      
      // Receive decoded audio frame
      this.frames.push(event.data.frame);
      
      // Start playback when buffer is full
      if (this.currentSamples() >= this.initialBufferSamples && !this.started) {
        this.started = true;
      }
    };
  }
  
  currentSamples(): number {
    return this.frames.reduce((sum, f) => sum + f.length, 0) - this.offsetInFirstBuffer;
  }
  
  process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const output = outputs[0][0];
    
    if (!this.started || this.frames.length === 0) {
      return true; // Output silence
    }
    
    let outIdx = 0;
    while (outIdx < output.length && this.frames.length > 0) {
      const first = this.frames[0];
      const toCopy = Math.min(
        first.length - this.offsetInFirstBuffer,
        output.length - outIdx
      );
      
      output.set(
        first.subarray(this.offsetInFirstBuffer, this.offsetInFirstBuffer + toCopy),
        outIdx
      );
      
      this.offsetInFirstBuffer += toCopy;
      outIdx += toCopy;
      
      if (this.offsetInFirstBuffer >= first.length) {
        this.frames.shift();
        this.offsetInFirstBuffer = 0;
      }
    }
    
    return true;
  }
}

registerProcessor('moshi-processor', MoshiProcessor);
```

### Audio Capture Hook (with opus-recorder)

The `useUserAudio` hook manages microphone capture and Opus encoding:

```typescript
// hooks/use-user-audio.ts
// Based on moshi/client/src/pages/Conversation/hooks/useUserAudio.ts

import { useCallback, useRef, useState } from 'react';
import Recorder from 'opus-recorder';
import encoderPath from 'opus-recorder/dist/encoderWorker.min.js?url';

export enum UserMediaStatus {
  IDLE = 'IDLE',
  WAITING_FOR_PERMISSION = 'WAITING_FOR_PERMISSION',
  RECORDING = 'RECORDING',
  STOPPED = 'STOPPED',
  ERROR = 'ERROR',
}

interface UseUserAudioOptions {
  onDataChunk?: (chunk: Uint8Array) => void;
  onRecordingStart?: () => void;
  onRecordingStop?: () => void;
}

export function useUserAudio({
  onDataChunk,
  onRecordingStart,
  onRecordingStop,
}: UseUserAudioOptions) {
  const [status, setStatus] = useState<UserMediaStatus>(UserMediaStatus.IDLE);
  const [error, setError] = useState<string | null>(null);
  const recorderRef = useRef<Recorder | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);

  const startRecording = useCallback(async () => {
    setStatus(UserMediaStatus.WAITING_FOR_PERMISSION);
    
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          channelCount: 1,
          sampleRate: { ideal: 24000 },
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      });
      
      const audioContext = new AudioContext();
      audioContextRef.current = audioContext;
      
      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      source.connect(analyser);
      
      const recorder = new Recorder({
        encoderPath,
        bufferLength: Math.round(960 * audioContext.sampleRate / 24000),
        encoderFrameSize: 20,
        encoderSampleRate: 24000,
        maxFramesPerPage: 2,
        numberOfChannels: 1,
        recordingGain: 1,
        resampleQuality: 3,
        encoderComplexity: 0,
        encoderApplication: 2049, // OPUS_APPLICATION_VOIP
        streamPages: true,
      });
      
      recorder.ondataavailable = (data: Uint8Array) => {
        onDataChunk?.(data);
      };
      
      recorder.onstart = () => {
        setStatus(UserMediaStatus.RECORDING);
        onRecordingStart?.();
      };
      
      recorder.onstop = () => {
        setStatus(UserMediaStatus.STOPPED);
        source.disconnect();
        onRecordingStop?.();
      };
      
      recorderRef.current = recorder;
      recorder.start();
      
      return { analyser, stream, source };
    } catch (err: any) {
      setError(err.name);
      setStatus(UserMediaStatus.ERROR);
      return null;
    }
  }, [onDataChunk, onRecordingStart, onRecordingStop]);

  const stopRecording = useCallback(() => {
    recorderRef.current?.stop();
    recorderRef.current = null;
  }, []);

  return {
    status,
    error,
    startRecording,
    stopRecording,
  };
}
```

### Authentication Client & Hook

The authentication system integrates with Better Auth for user authentication.

#### Auth Client Configuration

```typescript
// lib/auth/auth-client.ts
'use client';

import { createAuthClient } from 'better-auth/react';

// Create the Better Auth client
// baseURL defaults to same origin if not specified
export const authClient = createAuthClient({
  baseURL: process.env.NEXT_PUBLIC_AUTH_URL || undefined,
});

// Export commonly used functions
export const {
  signIn,
  signUp,
  signOut,
  useSession,
  getSession,
} = authClient;
```

#### Auth Hook

```typescript
// hooks/use-auth.ts
'use client';

import { useCallback, useMemo } from 'react';
import { useSession, signIn, signUp, signOut, getSession } from '@/lib/auth/auth-client';

export interface AuthUser {
  id: string;
  name?: string;
  email?: string;
  image?: string;
}

export interface UseAuthReturn {
  /** Whether the user is authenticated */
  isAuthenticated: boolean;
  /** Whether auth state is loading */
  isLoading: boolean;
  /** Current user data (null if not authenticated) */
  user: AuthUser | null;
  /** Sign in with email/password */
  signInWithEmail: (email: string, password: string) => Promise<void>;
  /** Sign up with email/password */
  signUpWithEmail: (email: string, password: string, name?: string) => Promise<void>;
  /** Sign out */
  signOut: () => Promise<void>;
  /** Get current session token (for WebSocket auth) */
  getSessionToken: () => Promise<string | null>;
}

export function useAuth(): UseAuthReturn {
  const { data: session, isPending } = useSession();
  
  const isAuthenticated = useMemo(() => {
    return !!session?.user;
  }, [session]);
  
  const user = useMemo<AuthUser | null>(() => {
    if (!session?.user) return null;
    return {
      id: session.user.id,
      name: session.user.name ?? undefined,
      email: session.user.email ?? undefined,
      image: session.user.image ?? undefined,
    };
  }, [session]);
  
  const signInWithEmail = useCallback(async (email: string, password: string) => {
    const result = await signIn.email({ email, password });
    if (result.error) {
      throw new Error(result.error.message || 'Sign in failed');
    }
  }, []);
  
  const signUpWithEmail = useCallback(async (email: string, password: string, name?: string) => {
    const result = await signUp.email({ email, password, name: name || email.split('@')[0] });
    if (result.error) {
      throw new Error(result.error.message || 'Sign up failed');
    }
  }, []);
  
  const handleSignOut = useCallback(async () => {
    await signOut();
  }, []);
  
  const getSessionToken = useCallback(async (): Promise<string | null> => {
    const session = await getSession();
    // The session token is stored in the cookie
    // For WebSocket auth, we can use the session ID or fetch the JWT
    if (!session?.data?.session) return null;
    
    // Return the session token for WebSocket authentication
    // This will be passed as auth_id query parameter
    return session.data.session.token || session.data.session.id;
  }, []);
  
  return {
    isAuthenticated,
    isLoading: isPending,
    user,
    signInWithEmail,
    signUpWithEmail,
    signOut: handleSignOut,
    getSessionToken,
  };
}
```

#### Using Auth with WebSocket

```typescript
// Example: Connecting with authentication
import { useAuth } from '@/hooks/use-auth';
import { useConnectionStore } from '@/lib/stores/connection-store';

function TranscriptionPage() {
  const { isAuthenticated, getSessionToken } = useAuth();
  const { connect, serverUrl } = useConnectionStore();
  
  const handleConnect = async () => {
    if (!isAuthenticated) {
      // Redirect to login or show auth modal
      return;
    }
    
    const token = await getSessionToken();
    await connect(serverUrl, { sessionToken: token });
  };
  
  return (
    <button onClick={handleConnect}>
      Connect to STT Server
    </button>
  );
}
```

---

## UI/UX Design

### Theme System

The application uses a modern blue pastel theme with full light/dark mode support, implemented via CSS variables and `next-themes`.

#### Theme Dependencies

```bash
# Theme provider
pnpm add next-themes
```

#### CSS Variables (globals.css)

```css
/* app/globals.css */
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  /* ============================================
   * KYUTAI STT - BLUE PASTEL THEME
   * Modern, accessible color palette
   * ============================================ */

  :root {
    /* === Base Colors === */
    --background: 210 40% 98%;           /* Soft blue-white */
    --foreground: 222 47% 11%;           /* Deep navy text */

    /* === Card & Surfaces === */
    --card: 210 40% 100%;                /* Pure white cards */
    --card-foreground: 222 47% 11%;
    --popover: 210 40% 100%;
    --popover-foreground: 222 47% 11%;

    /* === Primary - Blue Pastel === */
    --primary: 217 91% 60%;              /* Vibrant blue */
    --primary-foreground: 210 40% 98%;   /* White text on primary */

    /* === Secondary - Soft Blue === */
    --secondary: 214 32% 91%;            /* Light blue-gray */
    --secondary-foreground: 222 47% 11%;

    /* === Muted - Subtle Backgrounds === */
    --muted: 210 40% 96%;                /* Very light blue */
    --muted-foreground: 215 16% 47%;     /* Muted text */

    /* === Accent - Interactive Highlight === */
    --accent: 199 89% 48%;               /* Cyan accent */
    --accent-foreground: 210 40% 98%;

    /* === Destructive - Error States === */
    --destructive: 0 84% 60%;            /* Red */
    --destructive-foreground: 210 40% 98%;

    /* === Success - Positive States === */
    --success: 142 71% 45%;              /* Green */
    --success-foreground: 210 40% 98%;

    /* === Warning - Caution States === */
    --warning: 38 92% 50%;               /* Amber */
    --warning-foreground: 222 47% 11%;

    /* === Border & Input === */
    --border: 214 32% 91%;               /* Light border */
    --input: 214 32% 91%;                /* Input border */
    --ring: 217 91% 60%;                 /* Focus ring (primary) */

    /* === Audio Meter Colors === */
    --meter-low: 142 71% 45%;            /* Green - safe levels */
    --meter-mid: 38 92% 50%;             /* Amber - moderate */
    --meter-high: 0 84% 60%;             /* Red - clipping */
    --meter-background: 214 32% 91%;     /* Meter track */

    /* === Connection Status === */
    --status-connected: 142 71% 45%;     /* Green */
    --status-connecting: 38 92% 50%;     /* Amber */
    --status-disconnected: 215 16% 47%;  /* Gray */
    --status-error: 0 84% 60%;           /* Red */

    /* === VAD Indicator === */
    --vad-active: 142 71% 45%;           /* Green pulse */
    --vad-inactive: 215 16% 47%;         /* Gray */

    /* === Transcript === */
    --transcript-partial: 217 91% 60%;   /* Blue for live text */
    --transcript-final: 222 47% 11%;     /* Dark for final text */
    --confidence-high: 142 71% 45%;      /* Green */
    --confidence-medium: 38 92% 50%;     /* Amber */
    --confidence-low: 0 84% 60%;         /* Red */

    /* === Sidebar === */
    --sidebar: 210 40% 96%;
    --sidebar-foreground: 222 47% 11%;
    --sidebar-primary: 217 91% 60%;
    --sidebar-primary-foreground: 210 40% 98%;
    --sidebar-accent: 214 32% 91%;
    --sidebar-accent-foreground: 222 47% 11%;
    --sidebar-border: 214 32% 91%;
    --sidebar-ring: 217 91% 60%;

    /* === Chart Colors === */
    --chart-1: 217 91% 60%;              /* Primary blue */
    --chart-2: 199 89% 48%;              /* Cyan */
    --chart-3: 142 71% 45%;              /* Green */
    --chart-4: 38 92% 50%;               /* Amber */
    --chart-5: 280 65% 60%;              /* Purple */

    /* === Radius === */
    --radius: 0.625rem;                  /* 10px - slightly rounded */
  }

  .dark {
    /* === Base Colors === */
    --background: 222 47% 11%;           /* Deep navy */
    --foreground: 210 40% 98%;           /* Off-white text */

    /* === Card & Surfaces === */
    --card: 217 33% 17%;                 /* Elevated surface */
    --card-foreground: 210 40% 98%;
    --popover: 217 33% 17%;
    --popover-foreground: 210 40% 98%;

    /* === Primary - Blue Pastel (adjusted for dark) === */
    --primary: 213 94% 68%;              /* Lighter blue for dark mode */
    --primary-foreground: 222 47% 11%;   /* Dark text on primary */

    /* === Secondary === */
    --secondary: 217 33% 25%;            /* Dark blue-gray */
    --secondary-foreground: 210 40% 98%;

    /* === Muted === */
    --muted: 217 33% 20%;                /* Dark muted */
    --muted-foreground: 215 20% 65%;     /* Lighter muted text */

    /* === Accent === */
    --accent: 199 89% 48%;               /* Cyan (same) */
    --accent-foreground: 222 47% 11%;

    /* === Destructive === */
    --destructive: 0 62% 50%;            /* Darker red */
    --destructive-foreground: 210 40% 98%;

    /* === Success === */
    --success: 142 71% 40%;              /* Slightly darker green */
    --success-foreground: 210 40% 98%;

    /* === Warning === */
    --warning: 38 92% 45%;               /* Slightly darker amber */
    --warning-foreground: 222 47% 11%;

    /* === Border & Input === */
    --border: 217 33% 25%;               /* Dark border */
    --input: 217 33% 25%;
    --ring: 213 94% 68%;                 /* Focus ring */

    /* === Audio Meter Colors (Dark Mode) === */
    --meter-low: 142 71% 40%;
    --meter-mid: 38 92% 45%;
    --meter-high: 0 62% 50%;
    --meter-background: 217 33% 25%;

    /* === Connection Status (Dark Mode) === */
    --status-connected: 142 71% 40%;
    --status-connecting: 38 92% 45%;
    --status-disconnected: 215 20% 50%;
    --status-error: 0 62% 50%;

    /* === VAD Indicator (Dark Mode) === */
    --vad-active: 142 71% 40%;
    --vad-inactive: 215 20% 50%;

    /* === Transcript (Dark Mode) === */
    --transcript-partial: 213 94% 68%;
    --transcript-final: 210 40% 98%;
    --confidence-high: 142 71% 40%;
    --confidence-medium: 38 92% 45%;
    --confidence-low: 0 62% 50%;

    /* === Sidebar (Dark Mode) === */
    --sidebar: 217 33% 14%;
    --sidebar-foreground: 210 40% 98%;
    --sidebar-primary: 213 94% 68%;
    --sidebar-primary-foreground: 222 47% 11%;
    --sidebar-accent: 217 33% 20%;
    --sidebar-accent-foreground: 210 40% 98%;
    --sidebar-border: 217 33% 25%;
    --sidebar-ring: 213 94% 68%;

    /* === Chart Colors (Dark Mode) === */
    --chart-1: 213 94% 68%;
    --chart-2: 199 89% 55%;
    --chart-3: 142 71% 50%;
    --chart-4: 38 92% 55%;
    --chart-5: 280 65% 70%;
  }
}

@layer base {
  * {
    @apply border-border;
  }

  body {
    @apply bg-background text-foreground;
    font-feature-settings: "rlig" 1, "calt" 1;
  }
}

/* === Custom Utility Classes === */
@layer utilities {
  /* Audio meter gradient */
  .audio-meter-gradient {
    background: linear-gradient(
      to right,
      hsl(var(--meter-low)) 0%,
      hsl(var(--meter-low)) 60%,
      hsl(var(--meter-mid)) 80%,
      hsl(var(--meter-high)) 100%
    );
  }

  /* Pulsing animation for VAD */
  .vad-pulse {
    animation: vad-pulse 1.5s ease-in-out infinite;
  }

  @keyframes vad-pulse {
    0%, 100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.7;
      transform: scale(1.05);
    }
  }

  /* Live text cursor blink */
  .cursor-blink {
    animation: cursor-blink 1s step-end infinite;
  }

  @keyframes cursor-blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0; }
  }

  /* Smooth theme transition */
  .theme-transition {
    transition: background-color 0.3s ease, color 0.3s ease, border-color 0.3s ease;
  }
}
```

#### Tailwind Configuration

```typescript
// tailwind.config.ts
import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: [
    "./pages/**/*.{js,ts,jsx,tsx,mdx}",
    "./components/**/*.{js,ts,jsx,tsx,mdx}",
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        success: {
          DEFAULT: "hsl(var(--success))",
          foreground: "hsl(var(--success-foreground))",
        },
        warning: {
          DEFAULT: "hsl(var(--warning))",
          foreground: "hsl(var(--warning-foreground))",
        },
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        // Audio-specific colors
        meter: {
          low: "hsl(var(--meter-low))",
          mid: "hsl(var(--meter-mid))",
          high: "hsl(var(--meter-high))",
          background: "hsl(var(--meter-background))",
        },
        status: {
          connected: "hsl(var(--status-connected))",
          connecting: "hsl(var(--status-connecting))",
          disconnected: "hsl(var(--status-disconnected))",
          error: "hsl(var(--status-error))",
        },
        vad: {
          active: "hsl(var(--vad-active))",
          inactive: "hsl(var(--vad-inactive))",
        },
        transcript: {
          partial: "hsl(var(--transcript-partial))",
          final: "hsl(var(--transcript-final))",
        },
        confidence: {
          high: "hsl(var(--confidence-high))",
          medium: "hsl(var(--confidence-medium))",
          low: "hsl(var(--confidence-low))",
        },
        sidebar: {
          DEFAULT: "hsl(var(--sidebar))",
          foreground: "hsl(var(--sidebar-foreground))",
          primary: "hsl(var(--sidebar-primary))",
          "primary-foreground": "hsl(var(--sidebar-primary-foreground))",
          accent: "hsl(var(--sidebar-accent))",
          "accent-foreground": "hsl(var(--sidebar-accent-foreground))",
          border: "hsl(var(--sidebar-border))",
          ring: "hsl(var(--sidebar-ring))",
        },
        chart: {
          1: "hsl(var(--chart-1))",
          2: "hsl(var(--chart-2))",
          3: "hsl(var(--chart-3))",
          4: "hsl(var(--chart-4))",
          5: "hsl(var(--chart-5))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};

export default config;
```

#### Theme Provider Setup

```tsx
// components/providers/theme-provider.tsx
'use client';

import { ThemeProvider as NextThemesProvider } from 'next-themes';
import type { ThemeProviderProps } from 'next-themes';

export function ThemeProvider({ children, ...props }: ThemeProviderProps) {
  return (
    <NextThemesProvider
      attribute="class"
      defaultTheme="system"
      enableSystem
      disableTransitionOnChange={false}
      storageKey="kyutai-stt-theme"
      {...props}
    >
      {children}
    </NextThemesProvider>
  );
}
```

#### Theme Toggle Component

```tsx
// components/layout/theme-toggle.tsx
'use client';

import { useTheme } from 'next-themes';
import { useEffect, useState } from 'react';
import { Moon, Sun, Monitor } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils/cn';

export function ThemeToggle() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  // Prevent hydration mismatch
  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <Button variant="ghost" size="icon" className="h-9 w-9">
        <span className="h-4 w-4" />
      </Button>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button 
          variant="ghost" 
          size="icon" 
          className="h-9 w-9 theme-transition"
        >
          {resolvedTheme === 'dark' ? (
            <Moon className="h-4 w-4" />
          ) : (
            <Sun className="h-4 w-4" />
          )}
          <span className="sr-only">Toggle theme</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem 
          onClick={() => setTheme('light')}
          className={cn(theme === 'light' && 'bg-accent')}
        >
          <Sun className="mr-2 h-4 w-4" />
          Light
        </DropdownMenuItem>
        <DropdownMenuItem 
          onClick={() => setTheme('dark')}
          className={cn(theme === 'dark' && 'bg-accent')}
        >
          <Moon className="mr-2 h-4 w-4" />
          Dark
        </DropdownMenuItem>
        <DropdownMenuItem 
          onClick={() => setTheme('system')}
          className={cn(theme === 'system' && 'bg-accent')}
        >
          <Monitor className="mr-2 h-4 w-4" />
          System
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
```

#### Simple Theme Toggle (Icon Only)

```tsx
// components/layout/theme-toggle-simple.tsx
'use client';

import { useTheme } from 'next-themes';
import { useEffect, useState } from 'react';
import { Moon, Sun } from 'lucide-react';
import { Button } from '@/components/ui/button';

export function ThemeToggleSimple() {
  const { resolvedTheme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <Button variant="ghost" size="icon" className="h-9 w-9">
        <span className="h-4 w-4" />
      </Button>
    );
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-9 w-9 theme-transition"
      onClick={() => setTheme(resolvedTheme === 'dark' ? 'light' : 'dark')}
    >
      {resolvedTheme === 'dark' ? (
        <Sun className="h-4 w-4 transition-transform hover:rotate-45" />
      ) : (
        <Moon className="h-4 w-4 transition-transform hover:-rotate-12" />
      )}
      <span className="sr-only">Toggle theme</span>
    </Button>
  );
}
```

#### CSS Modules for Component-Specific Styles

```css
/* components/audio/audio-meter.module.css */
.meterContainer {
  position: relative;
  height: 8px;
  border-radius: 4px;
  overflow: hidden;
  background-color: hsl(var(--meter-background));
}

.meterFill {
  height: 100%;
  border-radius: 4px;
  transition: width 50ms ease-out;
}

.meterFillSafe {
  background-color: hsl(var(--meter-low));
}

.meterFillWarning {
  background-color: hsl(var(--meter-mid));
}

.meterFillDanger {
  background-color: hsl(var(--meter-high));
}

.meterGradient {
  background: linear-gradient(
    to right,
    hsl(var(--meter-low)) 0%,
    hsl(var(--meter-low)) 60%,
    hsl(var(--meter-mid)) 80%,
    hsl(var(--meter-high)) 100%
  );
}

.peakIndicator {
  position: absolute;
  top: 0;
  width: 2px;
  height: 100%;
  background-color: hsl(var(--foreground));
  opacity: 0.8;
  transition: left 100ms ease-out;
}
```

```css
/* components/connection/connection-status.module.css */
.statusDot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  transition: background-color 0.3s ease;
}

.connected {
  background-color: hsl(var(--status-connected));
  box-shadow: 0 0 8px hsl(var(--status-connected) / 0.5);
}

.connecting {
  background-color: hsl(var(--status-connecting));
  animation: pulse 1.5s ease-in-out infinite;
}

.disconnected {
  background-color: hsl(var(--status-disconnected));
}

.error {
  background-color: hsl(var(--status-error));
  animation: pulse 0.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.6;
    transform: scale(1.1);
  }
}
```

```css
/* components/audio/vad-indicator.module.css */
.vadContainer {
  display: flex;
  align-items: center;
  gap: 8px;
}

.vadDot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.vadActive {
  background-color: hsl(var(--vad-active));
  box-shadow: 0 0 12px hsl(var(--vad-active) / 0.6);
  animation: vadPulse 1s ease-in-out infinite;
}

.vadInactive {
  background-color: hsl(var(--vad-inactive));
}

@keyframes vadPulse {
  0%, 100% {
    transform: scale(1);
    box-shadow: 0 0 12px hsl(var(--vad-active) / 0.6);
  }
  50% {
    transform: scale(1.15);
    box-shadow: 0 0 20px hsl(var(--vad-active) / 0.8);
  }
}
```

```css
/* components/transcript/transcript.module.css */
.transcriptContainer {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.partialText {
  color: hsl(var(--transcript-partial));
  font-style: italic;
  opacity: 0.9;
}

.finalText {
  color: hsl(var(--transcript-final));
}

.cursor {
  display: inline-block;
  width: 2px;
  height: 1em;
  margin-left: 2px;
  background-color: hsl(var(--transcript-partial));
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

.confidenceHigh {
  background-color: hsl(var(--confidence-high) / 0.15);
}

.confidenceMedium {
  background-color: hsl(var(--confidence-medium) / 0.15);
}

.confidenceLow {
  background-color: hsl(var(--confidence-low) / 0.15);
}

.timestamp {
  font-family: ui-monospace, monospace;
  font-size: 0.75rem;
  color: hsl(var(--muted-foreground));
}
```

#### Using CSS Modules in Components

```tsx
// components/audio/audio-meter.tsx
'use client';

import { useAudioStore } from '@/lib/stores/audio-store';
import styles from './audio-meter.module.css';
import { cn } from '@/lib/utils/cn';

export function AudioMeter() {
  const { audioLevel, peakLevel } = useAudioStore();
  
  // Determine color based on level
  const levelClass = 
    audioLevel > 0.9 ? styles.meterFillDanger :
    audioLevel > 0.7 ? styles.meterFillWarning :
    styles.meterFillSafe;

  return (
    <div className="space-y-2">
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>Level</span>
        <span>{Math.round(audioLevel * 100)}%</span>
      </div>
      <div className={styles.meterContainer}>
        <div 
          className={cn(styles.meterFill, levelClass)}
          style={{ width: `${audioLevel * 100}%` }}
        />
        <div 
          className={styles.peakIndicator}
          style={{ left: `${peakLevel * 100}%` }}
        />
      </div>
    </div>
  );
}
```

#### Theme Color Palette Reference

| Token | Light Mode | Dark Mode | Usage |
|-------|------------|-----------|-------|
| `--primary` | `hsl(217 91% 60%)` | `hsl(213 94% 68%)` | Primary actions, links |
| `--background` | `hsl(210 40% 98%)` | `hsl(222 47% 11%)` | Page background |
| `--card` | `hsl(210 40% 100%)` | `hsl(217 33% 17%)` | Card surfaces |
| `--muted` | `hsl(210 40% 96%)` | `hsl(217 33% 20%)` | Subtle backgrounds |
| `--accent` | `hsl(199 89% 48%)` | `hsl(199 89% 48%)` | Highlights |
| `--success` | `hsl(142 71% 45%)` | `hsl(142 71% 40%)` | Success states |
| `--warning` | `hsl(38 92% 50%)` | `hsl(38 92% 45%)` | Warning states |
| `--destructive` | `hsl(0 84% 60%)` | `hsl(0 62% 50%)` | Error states |

---

### shadcn/ui Component Inventory

The application uses shadcn/ui as the component library. Below is the complete inventory of components required:

#### Core shadcn/ui Components (Install via CLI)

| Component | Usage | Install Command |
|-----------|-------|-----------------|
| **Button** | Recording controls, actions | `npx shadcn@latest add button` |
| **Card** | Transcript panel, settings cards | `npx shadcn@latest add card` |
| **Dialog** | Server config, confirmations | `npx shadcn@latest add dialog` |
| **Input** | URL, API key inputs | `npx shadcn@latest add input` |
| **Label** | Form labels | `npx shadcn@latest add label` |
| **Select** | Device selector, dropdowns | `npx shadcn@latest add select` |
| **Slider** | Volume, threshold controls | `npx shadcn@latest add slider` |
| **Switch** | Toggle settings | `npx shadcn@latest add switch` |
| **Badge** | Connection status, tags | `npx shadcn@latest add badge` |
| **Tooltip** | Help text, hints | `npx shadcn@latest add tooltip` |
| **Toast** | Notifications, errors | `npx shadcn@latest add toast` |
| **Toaster** | Toast container | `npx shadcn@latest add sonner` |
| **ScrollArea** | Transcript scrolling | `npx shadcn@latest add scroll-area` |
| **Separator** | Visual dividers | `npx shadcn@latest add separator` |
| **Skeleton** | Loading states | `npx shadcn@latest add skeleton` |
| **Progress** | Audio meter bars | `npx shadcn@latest add progress` |
| **Tabs** | Settings sections | `npx shadcn@latest add tabs` |
| **Alert** | Error messages, warnings | `npx shadcn@latest add alert` |
| **AlertDialog** | Destructive confirmations | `npx shadcn@latest add alert-dialog` |
| **DropdownMenu** | Context menus | `npx shadcn@latest add dropdown-menu` |
| **Sheet** | Mobile sidebar | `npx shadcn@latest add sheet` |
| **Popover** | Quick settings | `npx shadcn@latest add popover` |
| **Command** | Keyboard shortcuts | `npx shadcn@latest add command` |
| **Avatar** | User indicator | `npx shadcn@latest add avatar` |
| **Collapsible** | Expandable sections | `npx shadcn@latest add collapsible` |

#### Batch Install Command

```bash
npx shadcn@latest add button card dialog input label select slider switch badge tooltip sonner scroll-area separator skeleton progress tabs alert alert-dialog dropdown-menu sheet popover command avatar collapsible
```

---

### Complete Component Specifications

#### 1. TranscriptPanel Component

**File:** `components/transcript/transcript-panel.tsx`  
**shadcn/ui deps:** Card, CardContent, CardHeader, CardTitle, ScrollArea, Button, DropdownMenu

```tsx
'use client';

import { useRef, useEffect } from 'react';
import { useTranscriptStore } from '@/lib/stores/transcript-store';
import { TranscriptLine } from './transcript-line';
import { PartialText } from './partial-text';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Copy, Download, Trash2, MoreVertical, Mic } from 'lucide-react';
import { toast } from 'sonner';

export function TranscriptPanel() {
  const { segments, currentPartial, clearTranscript, exportTranscript } = useTranscriptStore();
  const scrollRef = useRef<HTMLDivElement>(null);
  
  // Auto-scroll to bottom on new content
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [segments, currentPartial]);
  
  const handleCopy = async () => {
    const text = exportTranscript();
    await navigator.clipboard.writeText(text);
    toast.success('Transcript copied to clipboard');
  };
  
  const handleExport = () => {
    const text = exportTranscript();
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `transcript-${new Date().toISOString()}.txt`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success('Transcript downloaded');
  };
  
  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-lg">
          <Mic className="h-5 w-5" />
          <span>Transcript</span>
          {currentPartial && (
            <span className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
          )}
        </CardTitle>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8">
              <MoreVertical className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={handleCopy}>
              <Copy className="mr-2 h-4 w-4" />
              Copy All
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleExport}>
              <Download className="mr-2 h-4 w-4" />
              Export as TXT
            </DropdownMenuItem>
            <DropdownMenuItem 
              onClick={clearTranscript}
              className="text-destructive"
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Clear Transcript
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </CardHeader>
      <CardContent className="flex-1 overflow-hidden">
        <ScrollArea className="h-[calc(100vh-280px)]" ref={scrollRef}>
          <div className="space-y-3 pr-4">
            {segments.length === 0 && !currentPartial && (
              <div className="text-center text-muted-foreground py-12">
                <Mic className="h-12 w-12 mx-auto mb-4 opacity-50" />
                <p>Start recording to see transcriptions</p>
              </div>
            )}
            {segments.map((segment) => (
              <TranscriptLine key={segment.id} segment={segment} />
            ))}
            {currentPartial && (
              <PartialText text={currentPartial.text} />
            )}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
```

#### 2. TranscriptLine Component

**File:** `components/transcript/transcript-line.tsx`  
**shadcn/ui deps:** Badge, Tooltip

```tsx
'use client';

import { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils/cn';
import { formatTimestamp } from '@/lib/utils/format-time';
import type { TranscriptSegment } from '@/types/transcript';

interface TranscriptLineProps {
  segment: TranscriptSegment;
  showTimestamp?: boolean;
}

export const TranscriptLine = memo(function TranscriptLine({ 
  segment, 
  showTimestamp = true 
}: TranscriptLineProps) {
  const confidenceColor = 
    segment.confidence >= 0.9 ? 'bg-green-500' :
    segment.confidence >= 0.7 ? 'bg-yellow-500' :
    'bg-red-500';
  
  return (
    <div className={cn(
      "group flex items-start gap-3 p-2 rounded-lg transition-colors",
      "hover:bg-muted/50"
    )}>
      {showTimestamp && (
        <span className="text-xs text-muted-foreground font-mono min-w-[60px]">
          {formatTimestamp(segment.timestamp)}
        </span>
      )}
      <div className="flex-1">
        <p className="text-sm leading-relaxed">{segment.text}</p>
        {segment.words && segment.words.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {segment.words.map((word, idx) => (
              <Tooltip key={idx}>
                <TooltipTrigger asChild>
                  <span className={cn(
                    "text-xs px-1 rounded cursor-help",
                    word.confidence >= 0.9 ? "bg-green-100 dark:bg-green-900/30" :
                    word.confidence >= 0.7 ? "bg-yellow-100 dark:bg-yellow-900/30" :
                    "bg-red-100 dark:bg-red-900/30"
                  )}>
                    {word.word}
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Confidence: {(word.confidence * 100).toFixed(0)}%</p>
                  <p className="text-xs text-muted-foreground">
                    {word.startTime.toFixed(2)}s - {word.endTime.toFixed(2)}s
                  </p>
                </TooltipContent>
              </Tooltip>
            ))}
          </div>
        )}
      </div>
      <Tooltip>
        <TooltipTrigger>
          <div className={cn("h-2 w-2 rounded-full", confidenceColor)} />
        </TooltipTrigger>
        <TooltipContent>
          Confidence: {(segment.confidence * 100).toFixed(0)}%
        </TooltipContent>
      </Tooltip>
    </div>
  );
});
```

#### 3. PartialText Component

**File:** `components/transcript/partial-text.tsx`  
**shadcn/ui deps:** None (custom styling)

```tsx
'use client';

import { cn } from '@/lib/utils/cn';

interface PartialTextProps {
  text: string;
}

export function PartialText({ text }: PartialTextProps) {
  return (
    <div className={cn(
      "flex items-start gap-3 p-2 rounded-lg",
      "bg-primary/5 border border-primary/20"
    )}>
      <span className="text-xs text-primary font-mono min-w-[60px]">
        live
      </span>
      <p className="text-sm leading-relaxed text-primary/80 italic">
        {text}
        <span className="inline-block w-2 h-4 ml-1 bg-primary/60 animate-pulse" />
      </p>
    </div>
  );
}
```

#### 4. AudioControls Component

**File:** `components/audio/audio-controls.tsx`  
**shadcn/ui deps:** Card, Button, Badge, Tooltip

```tsx
'use client';

import { useCallback, useEffect, useState } from 'react';
import { useAudioStore } from '@/lib/stores/audio-store';
import { useConnectionStore } from '@/lib/stores/connection-store';
import { useAudioCapture } from '@/hooks/use-audio-capture';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { 
  Mic, 
  MicOff, 
  Pause, 
  Play, 
  Square,
  Circle,
  Keyboard
} from 'lucide-react';
import { cn } from '@/lib/utils/cn';

export function AudioControls() {
  const { isRecording, isPaused, startRecording, stopRecording, pauseRecording, resumeRecording } = useAudioStore();
  const { status } = useConnectionStore();
  const { startCapture, stopCapture } = useAudioCapture();
  const [duration, setDuration] = useState(0);
  
  const isConnected = status === 'connected';
  
  // Duration timer
  useEffect(() => {
    let interval: NodeJS.Timeout;
    if (isRecording && !isPaused) {
      interval = setInterval(() => {
        setDuration(d => d + 1);
      }, 1000);
    }
    return () => clearInterval(interval);
  }, [isRecording, isPaused]);
  
  // Reset duration on stop
  useEffect(() => {
    if (!isRecording) setDuration(0);
  }, [isRecording]);
  
  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      
      if (e.code === 'Space' && e.ctrlKey) {
        e.preventDefault();
        if (!isRecording) {
          handleStart();
        } else {
          handleStop();
        }
      } else if (e.code === 'KeyP' && e.ctrlKey) {
        e.preventDefault();
        if (isRecording) {
          isPaused ? resumeRecording() : pauseRecording();
        }
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isRecording, isPaused]);
  
  const handleStart = useCallback(async () => {
    await startCapture();
    startRecording();
  }, [startCapture, startRecording]);
  
  const handleStop = useCallback(() => {
    stopCapture();
    stopRecording();
  }, [stopCapture, stopRecording]);
  
  const formatDuration = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };
  
  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center justify-between text-base">
          <span>Recording</span>
          {isRecording && (
            <Badge variant={isPaused ? 'secondary' : 'destructive'} className="gap-1">
              <Circle className={cn(
                "h-2 w-2 fill-current",
                !isPaused && "animate-pulse"
              )} />
              {isPaused ? 'Paused' : 'Recording'}
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Duration Display */}
        <div className="text-center">
          <span className="text-4xl font-mono font-bold tabular-nums">
            {formatDuration(duration)}
          </span>
        </div>
        
        {/* Control Buttons */}
        <div className="flex items-center justify-center gap-2">
          {!isRecording ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  size="lg"
                  onClick={handleStart}
                  disabled={!isConnected}
                  className="h-14 w-14 rounded-full"
                >
                  <Mic className="h-6 w-6" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Start Recording</p>
                <p className="text-xs text-muted-foreground">Ctrl + Space</p>
              </TooltipContent>
            </Tooltip>
          ) : (
            <>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    size="lg"
                    variant="outline"
                    onClick={isPaused ? resumeRecording : pauseRecording}
                    className="h-12 w-12 rounded-full"
                  >
                    {isPaused ? <Play className="h-5 w-5" /> : <Pause className="h-5 w-5" />}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>{isPaused ? 'Resume' : 'Pause'}</p>
                  <p className="text-xs text-muted-foreground">Ctrl + P</p>
                </TooltipContent>
              </Tooltip>
              
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    size="lg"
                    variant="destructive"
                    onClick={handleStop}
                    className="h-14 w-14 rounded-full"
                  >
                    <Square className="h-6 w-6 fill-current" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Stop Recording</p>
                  <p className="text-xs text-muted-foreground">Ctrl + Space</p>
                </TooltipContent>
              </Tooltip>
            </>
          )}
        </div>
        
        {/* Connection Warning */}
        {!isConnected && (
          <p className="text-xs text-center text-muted-foreground">
            Connect to server to start recording
          </p>
        )}
        
        {/* Keyboard Shortcuts Hint */}
        <div className="flex items-center justify-center gap-1 text-xs text-muted-foreground">
          <Keyboard className="h-3 w-3" />
          <span>Ctrl+Space to toggle</span>
        </div>
      </CardContent>
    </Card>
  );
}
```

#### 5. AudioMeter Component

**File:** `components/audio/audio-meter.tsx`  
**shadcn/ui deps:** Card, Progress, Tooltip

```tsx
'use client';

import { useAudioStore } from '@/lib/stores/audio-store';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Volume2, AlertTriangle } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

export function AudioMeter() {
  const { audioLevel, peakLevel, isRecording } = useAudioStore();
  
  // Convert to dB scale for display
  const rmsDb = 20 * Math.log10(Math.max(audioLevel, 0.0001));
  const peakDb = 20 * Math.log10(Math.max(peakLevel, 0.0001));
  
  // Normalize to 0-100 range (-60dB to 0dB)
  const rmsPercent = Math.max(0, Math.min(100, (rmsDb + 60) / 60 * 100));
  const peakPercent = Math.max(0, Math.min(100, (peakDb + 60) / 60 * 100));
  
  const isClipping = peakPercent > 95;
  const isLow = rmsPercent < 10 && isRecording;
  
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Volume2 className="h-4 w-4" />
          <span>Audio Level</span>
          {isClipping && (
            <Tooltip>
              <TooltipTrigger>
                <AlertTriangle className="h-4 w-4 text-destructive animate-pulse" />
              </TooltipTrigger>
              <TooltipContent>
                <p>Audio is clipping! Reduce input volume.</p>
              </TooltipContent>
            </Tooltip>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {/* RMS Level */}
        <div className="space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="text-muted-foreground">RMS</span>
            <span className="font-mono">{rmsDb.toFixed(1)} dB</span>
          </div>
          <div className="relative h-3 bg-muted rounded-full overflow-hidden">
            <div
              className={cn(
                "absolute inset-y-0 left-0 transition-all duration-75 rounded-full",
                rmsPercent > 90 ? "bg-red-500" :
                rmsPercent > 70 ? "bg-yellow-500" :
                "bg-green-500"
              )}
              style={{ width: `${rmsPercent}%` }}
            />
            {/* Threshold markers */}
            <div className="absolute inset-y-0 left-[70%] w-px bg-yellow-500/50" />
            <div className="absolute inset-y-0 left-[90%] w-px bg-red-500/50" />
          </div>
        </div>
        
        {/* Peak Level */}
        <div className="space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="text-muted-foreground">Peak</span>
            <span className="font-mono">{peakDb.toFixed(1)} dB</span>
          </div>
          <div className="relative h-3 bg-muted rounded-full overflow-hidden">
            <div
              className={cn(
                "absolute inset-y-0 left-0 transition-all duration-75 rounded-full",
                peakPercent > 95 ? "bg-red-500" :
                peakPercent > 80 ? "bg-yellow-500" :
                "bg-green-500"
              )}
              style={{ width: `${peakPercent}%` }}
            />
          </div>
        </div>
        
        {/* Status Messages */}
        {isLow && (
          <p className="text-xs text-yellow-600 dark:text-yellow-400">
            Low audio level detected
          </p>
        )}
        {isClipping && (
          <p className="text-xs text-destructive">
            Clipping detected - reduce volume
          </p>
        )}
        
        {/* Legend */}
        <div className="flex items-center justify-center gap-4 text-xs text-muted-foreground pt-2">
          <div className="flex items-center gap-1">
            <div className="h-2 w-2 rounded-full bg-green-500" />
            <span>Normal</span>
          </div>
          <div className="flex items-center gap-1">
            <div className="h-2 w-2 rounded-full bg-yellow-500" />
            <span>High</span>
          </div>
          <div className="flex items-center gap-1">
            <div className="h-2 w-2 rounded-full bg-red-500" />
            <span>Clip</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
```

#### 6. DeviceSelector Component

**File:** `components/audio/device-selector.tsx`  
**shadcn/ui deps:** Select, Label, Button, Skeleton

```tsx
'use client';

import { useEffect } from 'react';
import { useAudioStore } from '@/lib/stores/audio-store';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { RefreshCw, Mic } from 'lucide-react';

export function DeviceSelector() {
  const { 
    availableDevices, 
    selectedDeviceId, 
    selectDevice, 
    refreshDevices,
    isRecording 
  } = useAudioStore();
  
  useEffect(() => {
    refreshDevices();
  }, [refreshDevices]);
  
  const audioInputDevices = availableDevices.filter(
    device => device.kind === 'audioinput'
  );
  
  if (audioInputDevices.length === 0) {
    return (
      <div className="space-y-2">
        <Label>Microphone</Label>
        <Skeleton className="h-10 w-full" />
      </div>
    );
  }
  
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label htmlFor="device-select" className="flex items-center gap-2">
          <Mic className="h-4 w-4" />
          Microphone
        </Label>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6"
          onClick={refreshDevices}
          disabled={isRecording}
        >
          <RefreshCw className="h-3 w-3" />
        </Button>
      </div>
      <Select
        value={selectedDeviceId || undefined}
        onValueChange={selectDevice}
        disabled={isRecording}
      >
        <SelectTrigger id="device-select">
          <SelectValue placeholder="Select microphone" />
        </SelectTrigger>
        <SelectContent>
          {audioInputDevices.map((device) => (
            <SelectItem key={device.deviceId} value={device.deviceId}>
              {device.label || `Microphone ${device.deviceId.slice(0, 8)}`}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {isRecording && (
        <p className="text-xs text-muted-foreground">
          Stop recording to change device
        </p>
      )}
    </div>
  );
}
```

#### 7. ConnectionStatus Component

**File:** `components/connection/connection-status.tsx`  
**shadcn/ui deps:** Badge, Tooltip

```tsx
'use client';

import { useConnectionStore } from '@/lib/stores/connection-store';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Wifi, WifiOff, Loader2, AlertCircle, RefreshCw } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

const statusConfig = {
  disconnected: { 
    icon: WifiOff, 
    label: 'Disconnected', 
    variant: 'secondary' as const,
    description: 'Not connected to server'
  },
  connecting: { 
    icon: Loader2, 
    label: 'Connecting...', 
    variant: 'outline' as const,
    description: 'Establishing connection'
  },
  connected: { 
    icon: Wifi, 
    label: 'Connected', 
    variant: 'default' as const,
    description: 'Ready to transcribe'
  },
  reconnecting: { 
    icon: RefreshCw, 
    label: 'Reconnecting...', 
    variant: 'outline' as const,
    description: 'Connection lost, attempting to reconnect'
  },
  error: { 
    icon: AlertCircle, 
    label: 'Error', 
    variant: 'destructive' as const,
    description: 'Connection failed'
  },
};

export function ConnectionStatus() {
  const { status, reconnectAttempt, maxReconnectAttempts, error, serverUrl } = useConnectionStore();
  const config = statusConfig[status];
  const Icon = config.icon;
  
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge 
          variant={config.variant} 
          className={cn(
            "gap-1.5 cursor-help",
            status === 'connected' && "bg-green-600 hover:bg-green-700"
          )}
        >
          <Icon className={cn(
            "h-3 w-3",
            (status === 'connecting' || status === 'reconnecting') && "animate-spin"
          )} />
          <span>{config.label}</span>
          {status === 'reconnecting' && (
            <span className="text-xs opacity-75">
              ({reconnectAttempt}/{maxReconnectAttempts})
            </span>
          )}
        </Badge>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-xs">
        <p className="font-medium">{config.description}</p>
        <p className="text-xs text-muted-foreground mt-1 font-mono truncate">
          {serverUrl}
        </p>
        {error && (
          <p className="text-xs text-destructive mt-1">{error}</p>
        )}
      </TooltipContent>
    </Tooltip>
  );
}
```

#### 8. ServerConfig Component

**File:** `components/connection/server-config.tsx`  
**shadcn/ui deps:** Dialog, Button, Input, Label, Switch, Separator

```tsx
'use client';

import { useState } from 'react';
import { useConnectionStore } from '@/lib/stores/connection-store';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Separator } from '@/components/ui/separator';
import { Settings, Eye, EyeOff, Server, Key, User } from 'lucide-react';
import { toast } from 'sonner';

export function ServerConfig() {
  const { 
    serverUrl, 
    apiKey, 
    authId,
    status,
    setServerUrl, 
    setApiKey, 
    setAuthId,
    connect,
    disconnect 
  } = useConnectionStore();
  
  const [open, setOpen] = useState(false);
  const [localUrl, setLocalUrl] = useState(serverUrl);
  const [localApiKey, setLocalApiKey] = useState(apiKey || '');
  const [localAuthId, setLocalAuthId] = useState(authId || '');
  const [showApiKey, setShowApiKey] = useState(false);
  const [autoConnect, setAutoConnect] = useState(true);
  
  const isConnected = status === 'connected';
  
  const handleSave = async () => {
    // Validate URL
    try {
      const url = new URL(localUrl);
      if (!['ws:', 'wss:'].includes(url.protocol)) {
        toast.error('URL must use ws:// or wss:// protocol');
        return;
      }
    } catch {
      toast.error('Invalid URL format');
      return;
    }
    
    setServerUrl(localUrl);
    setApiKey(localApiKey || null);
    setAuthId(localAuthId || null);
    
    if (autoConnect) {
      try {
        await connect();
        toast.success('Connected to server');
      } catch (err) {
        toast.error('Failed to connect');
      }
    }
    
    setOpen(false);
  };
  
  const handleDisconnect = () => {
    disconnect();
    toast.info('Disconnected from server');
  };
  
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2">
          <Settings className="h-4 w-4" />
          <span className="hidden sm:inline">Server</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Server Configuration</DialogTitle>
          <DialogDescription>
            Configure the connection to the STT server.
          </DialogDescription>
        </DialogHeader>
        
        <div className="space-y-4 py-4">
          {/* Server URL */}
          <div className="space-y-2">
            <Label htmlFor="server-url" className="flex items-center gap-2">
              <Server className="h-4 w-4" />
              Server URL
            </Label>
            <Input
              id="server-url"
              value={localUrl}
              onChange={(e) => setLocalUrl(e.target.value)}
              placeholder="ws://localhost:8080/api/asr-streaming"
              className="font-mono text-sm"
            />
            <p className="text-xs text-muted-foreground">
              WebSocket endpoint (ws:// or wss://)
            </p>
          </div>
          
          <Separator />
          
          {/* API Key */}
          <div className="space-y-2">
            <Label htmlFor="api-key" className="flex items-center gap-2">
              <Key className="h-4 w-4" />
              API Key
              <span className="text-xs text-muted-foreground">(optional)</span>
            </Label>
            <div className="relative">
              <Input
                id="api-key"
                type={showApiKey ? 'text' : 'password'}
                value={localApiKey}
                onChange={(e) => setLocalApiKey(e.target.value)}
                placeholder="Enter API key"
                className="pr-10"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="absolute right-0 top-0 h-full px-3"
                onClick={() => setShowApiKey(!showApiKey)}
              >
                {showApiKey ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>
          
          {/* Auth ID */}
          <div className="space-y-2">
            <Label htmlFor="auth-id" className="flex items-center gap-2">
              <User className="h-4 w-4" />
              Auth ID
              <span className="text-xs text-muted-foreground">(optional)</span>
            </Label>
            <Input
              id="auth-id"
              value={localAuthId}
              onChange={(e) => setLocalAuthId(e.target.value)}
              placeholder="Enter auth ID"
            />
          </div>
          
          <Separator />
          
          {/* Auto-connect toggle */}
          <div className="flex items-center justify-between">
            <Label htmlFor="auto-connect" className="cursor-pointer">
              Connect after saving
            </Label>
            <Switch
              id="auto-connect"
              checked={autoConnect}
              onCheckedChange={setAutoConnect}
            />
          </div>
        </div>
        
        <DialogFooter className="flex-col sm:flex-row gap-2">
          {isConnected && (
            <Button 
              variant="outline" 
              onClick={handleDisconnect}
              className="w-full sm:w-auto"
            >
              Disconnect
            </Button>
          )}
          <Button onClick={handleSave} className="w-full sm:w-auto">
            {autoConnect ? 'Save & Connect' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

#### 9. VadIndicator Component

**File:** `components/audio/vad-indicator.tsx`  
**shadcn/ui deps:** Badge, Tooltip

```tsx
'use client';

import { useTranscriptStore } from '@/lib/stores/transcript-store';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Mic, MicOff, Volume2 } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

export function VadIndicator() {
  const { currentPartial, isProcessing } = useTranscriptStore();
  
  const isSpeaking = !!currentPartial;
  
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge 
          variant={isSpeaking ? 'default' : 'secondary'}
          className={cn(
            "gap-1.5 transition-colors",
            isSpeaking && "bg-green-600 hover:bg-green-700"
          )}
        >
          {isSpeaking ? (
            <>
              <Volume2 className="h-3 w-3 animate-pulse" />
              <span>Speaking</span>
            </>
          ) : isProcessing ? (
            <>
              <Mic className="h-3 w-3" />
              <span>Listening</span>
            </>
          ) : (
            <>
              <MicOff className="h-3 w-3" />
              <span>Silent</span>
            </>
          )}
        </Badge>
      </TooltipTrigger>
      <TooltipContent>
        <p>Voice Activity Detection</p>
        <p className="text-xs text-muted-foreground">
          {isSpeaking ? 'Speech detected' : 'Waiting for speech'}
        </p>
      </TooltipContent>
    </Tooltip>
  );
}
```

#### 10. Header Component

**File:** `components/layout/header.tsx`  
**shadcn/ui deps:** Button, Sheet, Separator

```tsx
'use client';

import Link from 'next/link';
import { ConnectionStatus } from '@/components/connection/connection-status';
import { ServerConfig } from '@/components/connection/server-config';
import { VadIndicator } from '@/components/audio/vad-indicator';
import { ThemeToggle } from '@/components/theme-toggle';
import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { Menu, Mic, Settings, History, Github } from 'lucide-react';

export function Header() {
  return (
    <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="container flex h-14 items-center">
        {/* Logo */}
        <Link href="/" className="flex items-center gap-2 mr-6">
          <Mic className="h-6 w-6 text-primary" />
          <span className="font-bold text-lg hidden sm:inline">Kyutai STT</span>
        </Link>
        
        {/* Desktop Navigation */}
        <nav className="hidden md:flex items-center gap-4 flex-1">
          <Link href="/settings">
            <Button variant="ghost" size="sm" className="gap-2">
              <Settings className="h-4 w-4" />
              Settings
            </Button>
          </Link>
          <Link href="/history">
            <Button variant="ghost" size="sm" className="gap-2">
              <History className="h-4 w-4" />
              History
            </Button>
          </Link>
        </nav>
        
        {/* Status Indicators */}
        <div className="flex items-center gap-2 ml-auto">
          <div className="hidden sm:flex items-center gap-2">
            <VadIndicator />
            <Separator orientation="vertical" className="h-6" />
          </div>
          <ConnectionStatus />
          <ServerConfig />
          <ThemeToggle />
          
          {/* Mobile Menu */}
          <Sheet>
            <SheetTrigger asChild className="md:hidden">
              <Button variant="ghost" size="icon">
                <Menu className="h-5 w-5" />
              </Button>
            </SheetTrigger>
            <SheetContent side="right">
              <SheetHeader>
                <SheetTitle>Menu</SheetTitle>
              </SheetHeader>
              <nav className="flex flex-col gap-2 mt-4">
                <Link href="/settings">
                  <Button variant="ghost" className="w-full justify-start gap-2">
                    <Settings className="h-4 w-4" />
                    Settings
                  </Button>
                </Link>
                <Link href="/history">
                  <Button variant="ghost" className="w-full justify-start gap-2">
                    <History className="h-4 w-4" />
                    History
                  </Button>
                </Link>
                <Separator className="my-2" />
                <a 
                  href="https://github.com/kyutai/stt-web-client" 
                  target="_blank" 
                  rel="noopener noreferrer"
                >
                  <Button variant="ghost" className="w-full justify-start gap-2">
                    <Github className="h-4 w-4" />
                    GitHub
                  </Button>
                </a>
              </nav>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </header>
  );
}
```

#### 11. ThemeToggle Component

**File:** `components/layout/theme-toggle.tsx`  
**shadcn/ui deps:** Button, DropdownMenu

> **See full implementation:** [Theme Toggle Component](#theme-toggle-component) in the UI/UX Design section above. Two variants are provided:
> - `ThemeToggle` - Dropdown with Light/Dark/System options
> - `ThemeToggleSimple` - Single button toggle

#### 12. Footer Component

**File:** `components/layout/footer.tsx`  
**shadcn/ui deps:** Separator

```tsx
import { Separator } from '@/components/ui/separator';
import { Heart, Github } from 'lucide-react';

export function Footer() {
  return (
    <footer className="border-t py-4 mt-auto">
      <div className="container flex flex-col sm:flex-row items-center justify-between gap-2 text-sm text-muted-foreground">
        <p className="flex items-center gap-1">
          Real-time Speech-to-Text powered by
          <a 
            href="https://kyutai.org" 
            target="_blank" 
            rel="noopener noreferrer"
            className="font-medium text-foreground hover:underline"
          >
            Kyutai
          </a>
        </p>
        <div className="flex items-center gap-4">
          <a 
            href="https://github.com/kyutai/stt-web-client"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 hover:text-foreground transition-colors"
          >
            <Github className="h-4 w-4" />
            <span>Source</span>
          </a>
          <Separator orientation="vertical" className="h-4" />
          <span className="flex items-center gap-1">
            Made with <Heart className="h-3 w-3 text-red-500 fill-red-500" />
          </span>
        </div>
      </div>
    </footer>
  );
}
```

---

### Additional UI Components

#### 13. ErrorAlert Component

**File:** `components/error-alert.tsx`  
**shadcn/ui deps:** Alert, AlertDialog

```tsx
'use client';

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from '@/components/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { AlertCircle, XCircle } from 'lucide-react';

interface ErrorAlertProps {
  title: string;
  message: string;
  onDismiss?: () => void;
}

export function ErrorAlert({ title, message, onDismiss }: ErrorAlertProps) {
  return (
    <Alert variant="destructive">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  );
}

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  onConfirm: () => void;
  confirmText?: string;
  cancelText?: string;
  destructive?: boolean;
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  onConfirm,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  destructive = false,
}: ConfirmDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{cancelText}</AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className={destructive ? 'bg-destructive text-destructive-foreground hover:bg-destructive/90' : ''}
          >
            {confirmText}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

#### 14. LoadingSpinner Component

**File:** `components/loading-spinner.tsx`  
**shadcn/ui deps:** None (custom)

```tsx
import { Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils/cn';

interface LoadingSpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const sizeClasses = {
  sm: 'h-4 w-4',
  md: 'h-6 w-6',
  lg: 'h-8 w-8',
};

export function LoadingSpinner({ size = 'md', className }: LoadingSpinnerProps) {
  return (
    <Loader2 className={cn('animate-spin', sizeClasses[size], className)} />
  );
}
```

#### 15. EmptyState Component

**File:** `components/empty-state.tsx`  
**shadcn/ui deps:** Button

```tsx
import { Button } from '@/components/ui/button';
import { LucideIcon } from 'lucide-react';

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <Icon className="h-12 w-12 text-muted-foreground/50 mb-4" />
      <h3 className="text-lg font-medium mb-1">{title}</h3>
      <p className="text-sm text-muted-foreground mb-4 max-w-sm">{description}</p>
      {action && (
        <Button onClick={action.onClick}>{action.label}</Button>
      )}
    </div>
  );
}
```

---

### Component Hierarchy Diagram

```
App
├── Providers
│   ├── ThemeProvider
│   ├── TooltipProvider
│   └── Toaster (Sonner)
├── Header
│   ├── Logo
│   ├── Navigation (Desktop)
│   │   ├── Settings Link
│   │   └── History Link
│   ├── VadIndicator
│   ├── ConnectionStatus
│   ├── ServerConfig (Dialog)
│   ├── ThemeToggle
│   └── MobileMenu (Sheet)
├── Main
│   ├── Sidebar (lg:col-span-1)
│   │   ├── AudioControls (Card)
│   │   ├── AudioMeter (Card)
│   │   └── DeviceSelector
│   └── Content (lg:col-span-3)
│       └── TranscriptPanel (Card)
│           ├── TranscriptLine[]
│           └── PartialText
└── Footer
```

---

### Responsive Breakpoints

| Breakpoint | Width | Layout |
|------------|-------|--------|
| `sm` | 640px | Single column, compact controls |
| `md` | 768px | Show desktop nav |
| `lg` | 1024px | 4-column grid (1 sidebar + 3 content) |
| `xl` | 1280px | Max container width |

---

### Updated Layout Design

**File:** `app/page.tsx`

```tsx
import { Header } from '@/components/layout/header';
import { Footer } from '@/components/layout/footer';
import { TranscriptPanel } from '@/components/transcript/transcript-panel';
import { AudioControls } from '@/components/audio/audio-controls';
import { AudioMeter } from '@/components/audio/audio-meter';
import { DeviceSelector } from '@/components/audio/device-selector';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Settings2 } from 'lucide-react';

export default function HomePage() {
  return (
    <div className="min-h-screen bg-background flex flex-col">
      <Header />
      
      {/* Main Content */}
      <main className="container py-6 flex-1">
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
          {/* Sidebar - Audio Controls */}
          <aside className="lg:col-span-1 space-y-4">
            <AudioControls />
            <AudioMeter />
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="flex items-center gap-2 text-base">
                  <Settings2 className="h-4 w-4" />
                  Audio Settings
                </CardTitle>
              </CardHeader>
              <CardContent>
                <DeviceSelector />
              </CardContent>
            </Card>
          </aside>
          
          {/* Main - Transcript */}
          <div className="lg:col-span-3">
            <TranscriptPanel />
          </div>
        </div>
      </main>
      
      <Footer />
    </div>
  );
}
```

**File:** `app/layout.tsx`

```tsx
import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import { ThemeProvider } from '@/components/theme-provider';
import { TooltipProvider } from '@/components/ui/tooltip';
import { Toaster } from '@/components/ui/sonner';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'Kyutai STT - Real-time Speech-to-Text',
  description: 'Stream your microphone to a speech-to-text server and see transcriptions in real-time.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={inter.className}>
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <TooltipProvider delayDuration={300}>
            {children}
            <Toaster position="bottom-right" />
          </TooltipProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
```

---

## Security Considerations

### Authentication Architecture

The moshi-server supports multiple authentication methods, allowing flexibility for different deployment scenarios:

#### 1. Legacy API Key Authentication

- **Header**: `kyutai-api-key`
- **Query Parameter**: `auth_id`
- **Configuration**: Set via `MOSHI_API_KEY` environment variable (comma-separated list)
- **Use Case**: Programmatic API access, service-to-service communication

```typescript
// Example: API key via header
const ws = new WebSocket('wss://stt.example.com/api/asr-streaming');
// Set header via custom protocol or use query param
const wsWithAuth = new WebSocket('wss://stt.example.com/api/asr-streaming?auth_id=your-api-key');
```

#### 2. Better Auth JWT Authentication

- **Header**: `Authorization: Bearer <jwt-token>`
- **Cookie**: `better-auth.session_token`
- **Configuration**: Set `BETTER_AUTH_SECRET` environment variable (must match auth server)
- **Use Case**: Web client user authentication

```typescript
// Example: JWT via Authorization header
const token = await getSessionToken(); // From Better Auth client
const ws = new WebSocket('wss://stt.example.com/api/asr-streaming');
// Note: WebSocket API doesn't support custom headers directly
// Use query param or cookie-based auth instead
const wsWithJwt = new WebSocket(`wss://stt.example.com/api/asr-streaming?auth_id=${token}`);
```

#### 3. Session Cookie Authentication

- **Cookie Name**: `better-auth.session_token`
- **Automatic**: Sent by browser for same-origin requests
- **Use Case**: Seamless web client authentication when auth server is same-origin

### JWT Claims Structure (Better Auth)

The moshi-server validates JWTs with the following structure:

```typescript
interface BetterAuthClaims {
  session: {
    id: string;           // Session ID
    userId: string;       // User ID
    createdAt: string;    // ISO 8601 timestamp
    updatedAt: string;    // ISO 8601 timestamp
    expiresAt: string;    // ISO 8601 timestamp (session expiry)
    token?: string;       // Session token
    ipAddress?: string;   // Client IP
    userAgent?: string;   // Client user agent
  };
  user: {
    id: string;           // User ID
    name?: string;        // Display name
    email?: string;       // Email address
    emailVerified?: boolean;
    image?: string;       // Avatar URL
  };
  iat?: number;           // Issued at (Unix timestamp)
  exp?: number;           // Expiration (Unix timestamp)
}
```

### Authentication Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌──────────────────┐
│   Web Client    │     │  Better Auth    │     │   moshi-server   │
│   (Next.js)     │     │     Server      │     │     (Rust)       │
└────────┬────────┘     └────────┬────────┘     └────────┬─────────┘
         │                       │                        │
         │  1. Sign In           │                        │
         │──────────────────────>│                        │
         │                       │                        │
         │  2. JWT Cookie        │                        │
         │<──────────────────────│                        │
         │                       │                        │
         │  3. WebSocket + JWT (via query param or cookie)│
         │───────────────────────────────────────────────>│
         │                       │                        │
         │                       │  4. Validate JWT       │
         │                       │  (BETTER_AUTH_SECRET)  │
         │                       │                        │
         │  5. Connection Established                     │
         │<───────────────────────────────────────────────│
         │                       │                        │
```

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `MOSHI_API_KEY` | Comma-separated list of valid API keys | No |
| `BETTER_AUTH_SECRET` | Shared secret for JWT validation (min 32 chars) | No (required for JWT auth) |
| `VITE_AUTH_URL` | Better Auth server URL (client-side) | No (defaults to same origin) |

### API Key Management Best Practices

1. **Never expose API keys in client-side code** - Use environment variables
2. **Use Better Auth for web users** - API keys are for programmatic access
3. **Allow user-provided keys** stored in localStorage (for development/testing)
4. **Implement key rotation** support
5. **Use HTTPS/WSS in production** - Tokens are transmitted in URLs/headers

### WebSocket Security

```typescript
// lib/websocket/security.ts
export function validateServerUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    // Only allow ws:// and wss:// protocols
    if (!['ws:', 'wss:'].includes(parsed.protocol)) {
      return false;
    }
    // Block localhost in production
    if (process.env.NODE_ENV === 'production' && 
        ['localhost', '127.0.0.1', '::1'].includes(parsed.hostname)) {
      return false;
    }
    return true;
  } catch {
    return false;
  }
}

export function sanitizeApiKey(key: string): string {
  // Remove any whitespace and validate format
  return key.trim().replace(/[^a-zA-Z0-9-_]/g, '');
}

// Build authenticated WebSocket URL
export function buildAuthenticatedWsUrl(
  baseUrl: string,
  options: {
    apiKey?: string;
    sessionToken?: string;
  }
): string {
  const url = new URL(baseUrl);
  
  // Prefer session token (Better Auth JWT) over API key
  if (options.sessionToken) {
    url.searchParams.set('auth_id', options.sessionToken);
  } else if (options.apiKey) {
    url.searchParams.set('auth_id', options.apiKey);
  }
  
  return url.toString();
}
```

### Content Security Policy

```typescript
// next.config.ts
const securityHeaders = [
  {
    key: 'Content-Security-Policy',
    value: [
      "default-src 'self'",
      "script-src 'self' 'unsafe-eval'", // Required for AudioWorklet
      "worker-src 'self' blob:",
      "connect-src 'self' ws: wss:",
      "style-src 'self' 'unsafe-inline'",
    ].join('; '),
  },
];
```

---

## Performance Optimization

### Audio Processing

1. **Use AudioWorklet** instead of ScriptProcessorNode (deprecated)
2. **Buffer audio chunks** to reduce WebSocket message frequency
3. **Implement backpressure** when WebSocket is slow

### React Optimization

1. **Memoize transcript components** to prevent unnecessary re-renders
2. **Use virtualized lists** for long transcripts
3. **Debounce audio level updates** for visualization

```typescript
// hooks/use-debounced-audio-level.ts
import { useMemo } from 'react';
import { useAudioStore } from '@/lib/stores/audio-store';

export function useDebouncedAudioLevel(intervalMs = 50) {
  const { audioLevel, peakLevel } = useAudioStore();
  
  // Throttle updates to reduce re-renders
  const throttledLevel = useMemo(() => {
    return {
      audioLevel: Math.round(audioLevel * 100) / 100,
      peakLevel: Math.round(peakLevel * 100) / 100,
    };
  }, [
    Math.round(audioLevel * 20), // Update every 5% change
    Math.round(peakLevel * 20),
  ]);
  
  return throttledLevel;
}
```

### Bundle Optimization

```typescript
// next.config.ts
const nextConfig = {
  experimental: {
    optimizePackageImports: ['lucide-react', '@msgpack/msgpack'],
  },
  webpack: (config) => {
    // Optimize MessagePack bundle
    config.resolve.alias['@msgpack/msgpack'] = '@msgpack/msgpack/dist.es5+esm';
    return config;
  },
};
```

---

## Testing Strategy

### Unit Tests (Vitest)

```typescript
// __tests__/lib/websocket/message-encoder.test.ts
import { describe, it, expect } from 'vitest';
import { encodeAudioChunk } from '@/lib/websocket/message-encoder';

describe('encodeAudioChunk', () => {
  it('should encode audio samples to MessagePack', () => {
    const chunk = {
      samples: new Float32Array([0.1, -0.2, 0.3]),
      sampleRate: 24000,
      timestamp: 1234567890,
    };
    
    const encoded = encodeAudioChunk(chunk);
    
    expect(encoded).toBeInstanceOf(Uint8Array);
    expect(encoded.length).toBeGreaterThan(0);
  });
});
```

### Integration Tests (Playwright)

```typescript
// e2e/transcription.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Transcription Flow', () => {
  test('should connect to server and display transcripts', async ({ page }) => {
    await page.goto('/');
    
    // Configure server
    await page.click('[data-testid="server-config"]');
    await page.fill('[data-testid="server-url"]', 'ws://localhost:8080/api/asr-streaming');
    await page.click('[data-testid="connect-button"]');
    
    // Wait for connection
    await expect(page.locator('[data-testid="connection-status"]'))
      .toHaveText('Connected');
    
    // Start recording (mocked audio)
    await page.click('[data-testid="start-recording"]');
    
    // Verify transcript appears
    await expect(page.locator('[data-testid="transcript-panel"]'))
      .toContainText('', { timeout: 10000 });
  });
});
```

### Mock Server for Testing

```typescript
// __tests__/mocks/websocket-server.ts
import { WebSocketServer } from 'ws';

export function createMockSTTServer(port: number) {
  const wss = new WebSocketServer({ port });
  
  wss.on('connection', (ws) => {
    ws.on('message', (data) => {
      // Simulate transcription response
      setTimeout(() => {
        ws.send(JSON.stringify({
          type: 'partial',
          text: 'test transcription',
          confidence: 0.9,
          timestamp: Date.now(),
        }));
      }, 100);
    });
  });
  
  return wss;
}
```

---

## Deployment Strategy

### Vercel Deployment

```json
// vercel.json
{
  "buildCommand": "pnpm build",
  "outputDirectory": ".next",
  "framework": "nextjs",
  "regions": ["iad1"],
  "headers": [
    {
      "source": "/(.*)",
      "headers": [
        {
          "key": "X-Content-Type-Options",
          "value": "nosniff"
        }
      ]
    }
  ]
}
```

### Docker Deployment

```dockerfile
# Dockerfile
FROM node:20-alpine AS base
RUN corepack enable pnpm

FROM base AS deps
WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN pnpm build

FROM base AS runner
WORKDIR /app
ENV NODE_ENV=production
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

EXPOSE 3000
CMD ["node", "server.js"]
```

---

## GitHub Issues & Branches

### Epic Structure

```
Epic: Next.js Real-Time STT Web Client
├── Frontend Foundation (Issues #1-5)
├── Audio Pipeline (Issues #6-10)
├── WebSocket Integration (Issues #11-15)
├── UI Components (Issues #16-25)
├── Testing & Quality (Issues #26-30)
└── Deployment & Documentation (Issues #31-35)
```

---

### Frontend Foundation

#### Issue #1: Project Scaffolding
**Branch:** `feature/project-scaffolding`
**Labels:** `frontend`, `setup`, `priority:high`

**Description:**
Initialize Next.js 15 project with TypeScript, Tailwind CSS, and Biome.

**Tasks:**
- [ ] Create Next.js project with App Router
- [ ] Configure TypeScript strict mode
- [ ] Set up Tailwind CSS with custom theme
- [ ] Configure Biome for linting/formatting
- [ ] Add base directory structure
- [ ] Create initial `package.json` scripts

**Acceptance Criteria:**
- `pnpm dev` starts development server
- `pnpm build` completes without errors
- `pnpm lint` runs Biome checks

---

#### Issue #2: shadcn/ui Setup
**Branch:** `feature/shadcn-setup`
**Labels:** `frontend`, `ui`, `priority:high`

**Description:**
Install and configure shadcn/ui component library.

**Tasks:**
- [ ] Initialize shadcn/ui with CLI
- [ ] Install core components (Button, Card, Input, Dialog, etc.)
- [ ] Configure dark/light theme support
- [ ] Set up theme provider
- [ ] Create `cn()` utility function

**Acceptance Criteria:**
- All core components render correctly
- Theme toggle works
- Components follow design system

---

#### Issue #3: State Management Setup
**Branch:** `feature/state-management`
**Labels:** `frontend`, `state`, `priority:high`

**Description:**
Implement Zustand stores for application state.

**Tasks:**
- [ ] Create `audio-store.ts`
- [ ] Create `transcript-store.ts`
- [ ] Create `connection-store.ts`
- [ ] Add TypeScript types for all stores
- [ ] Implement store persistence (localStorage)

**Acceptance Criteria:**
- Stores are properly typed
- State persists across page reloads
- DevTools integration works

---

#### Issue #4: Environment Configuration
**Branch:** `feature/env-config`
**Labels:** `frontend`, `config`, `priority:medium`

**Description:**
Set up environment variables and configuration system.

**Tasks:**
- [ ] Create `.env.example` with all variables
- [ ] Add environment validation
- [ ] Create configuration constants file
- [ ] Document all environment variables

**Acceptance Criteria:**
- App fails gracefully with missing env vars
- All configurable values use env vars

---

#### Issue #5: Layout Components
**Branch:** `feature/layout-components`
**Labels:** `frontend`, `ui`, `priority:medium`

**Description:**
Create base layout components (Header, Footer, Sidebar).

**Tasks:**
- [ ] Create `Header` component with logo and controls
- [ ] Create `Footer` component with status info
- [ ] Create responsive layout wrapper
- [ ] Implement mobile navigation

**Acceptance Criteria:**
- Layout is responsive
- Navigation works on all screen sizes

---

### Audio Pipeline

#### Issue #6: AudioWorklet Processor
**Branch:** `feature/audio-worklet`
**Labels:** `frontend`, `audio`, `priority:critical`

**Description:**
Implement AudioWorklet processor for audio capture.

**Tasks:**
- [ ] Create `audio-processor.js` worklet script
- [ ] Implement chunk buffering (1920 samples)
- [ ] Add RMS/peak level calculation
- [ ] Handle sample rate conversion
- [ ] Add worklet registration utility

**Acceptance Criteria:**
- Worklet processes audio without dropouts
- Audio levels are accurate
- Chunks are correctly sized

---

#### Issue #7: Microphone Capture Hook
**Branch:** `feature/mic-capture-hook`
**Labels:** `frontend`, `audio`, `priority:critical`

**Description:**
Create React hook for microphone capture management.

**Tasks:**
- [ ] Implement `useAudioCapture` hook
- [ ] Handle `getUserMedia` permissions
- [ ] Connect AudioContext and WorkletNode
- [ ] Implement start/stop/pause controls
- [ ] Add error handling for device access

**Acceptance Criteria:**
- Microphone access works across browsers
- Graceful handling of permission denial
- Clean resource cleanup on unmount

---

#### Issue #8: Audio Device Enumeration
**Branch:** `feature/audio-devices`
**Labels:** `frontend`, `audio`, `priority:medium`

**Description:**
Implement audio device listing and selection.

**Tasks:**
- [ ] Create `useAudioDevices` hook
- [ ] Enumerate available input devices
- [ ] Handle device change events
- [ ] Persist selected device preference

**Acceptance Criteria:**
- All microphones are listed
- Device selection persists
- Hot-plug detection works

---

#### Issue #9: Audio Resampling
**Branch:** `feature/audio-resampling`
**Labels:** `frontend`, `audio`, `priority:medium`

**Description:**
Implement client-side audio resampling to 24kHz.

**Tasks:**
- [ ] Create `LinearResampler` class
- [ ] Handle various input sample rates
- [ ] Optimize for real-time processing
- [ ] Add unit tests

**Acceptance Criteria:**
- Resampling is accurate
- No audible artifacts
- Performance is acceptable

---

#### Issue #10: Audio Level Visualization
**Branch:** `feature/audio-visualization`
**Labels:** `frontend`, `audio`, `ui`, `priority:medium`

**Description:**
Create real-time audio level visualization components.

**Tasks:**
- [ ] Create `AudioMeter` component
- [ ] Implement dB scale conversion
- [ ] Add peak hold indicator
- [ ] Create sparkline history view

**Acceptance Criteria:**
- Levels update in real-time
- Visualization is smooth
- dB readings are accurate

---

### WebSocket Integration

#### Issue #11: WebSocket Client Class
**Branch:** `feature/websocket-client`
**Labels:** `frontend`, `websocket`, `priority:critical`

**Description:**
Implement WebSocket client for STT server communication.

**Tasks:**
- [ ] Create `STTWebSocketClient` class
- [ ] Implement connection with auth headers
- [ ] Handle binary and text messages
- [ ] Add connection state management
- [ ] Implement auto-reconnection

**Acceptance Criteria:**
- Connects to Rust STT server
- Handles all message types
- Reconnects on disconnect

---

#### Issue #12: MessagePack Encoding
**Branch:** `feature/msgpack-encoding`
**Labels:** `frontend`, `websocket`, `priority:critical`

**Description:**
Implement MessagePack encoding for audio chunks.

**Tasks:**
- [ ] Install `@msgpack/msgpack`
- [ ] Create `encodeAudioChunk` function
- [ ] Match Rust server's expected format
- [ ] Add encoding benchmarks

**Acceptance Criteria:**
- Encoded messages are accepted by server
- Encoding is performant

---

#### Issue #13: Message Type Definitions
**Branch:** `feature/message-types`
**Labels:** `frontend`, `websocket`, `types`, `priority:high`

**Description:**
Define TypeScript types for all WebSocket messages.

**Tasks:**
- [ ] Define `PartialTranscription` type
- [ ] Define `FinalTranscription` type
- [ ] Define `ErrorMessage` type
- [ ] Define `StatusMessage` type
- [ ] Create type guards for message parsing

**Acceptance Criteria:**
- All message types are fully typed
- Type guards work correctly

---

#### Issue #14: WebSocket React Hook
**Branch:** `feature/websocket-hook`
**Labels:** `frontend`, `websocket`, `priority:high`

**Description:**
Create React hook for WebSocket management.

**Tasks:**
- [ ] Create `useWebSocket` hook
- [ ] Integrate with connection store
- [ ] Handle lifecycle (connect/disconnect)
- [ ] Expose send function

**Acceptance Criteria:**
- Hook manages WebSocket lifecycle
- State syncs with store

---

#### Issue #15: Connection Error Handling
**Branch:** `feature/connection-errors`
**Labels:** `frontend`, `websocket`, `priority:high`

**Description:**
Implement comprehensive error handling for WebSocket.

**Tasks:**
- [ ] Handle authentication errors (401)
- [ ] Handle connection timeouts
- [ ] Handle network errors
- [ ] Display user-friendly error messages
- [ ] Implement retry logic

**Acceptance Criteria:**
- All error cases are handled
- Users see helpful error messages

---

### UI Components

#### Issue #16: Transcript Panel Component
**Branch:** `feature/transcript-panel`
**Labels:** `frontend`, `ui`, `priority:high`

**Description:**
Create main transcript display panel.

**Tasks:**
- [ ] Create `TranscriptPanel` component
- [ ] Implement auto-scroll behavior
- [ ] Add copy-to-clipboard functionality
- [ ] Style partial vs final text differently

**Acceptance Criteria:**
- Transcripts display correctly
- Auto-scroll works
- Copy function works

---

#### Issue #17: Transcript Line Component
**Branch:** `feature/transcript-line`
**Labels:** `frontend`, `ui`, `priority:high`

**Description:**
Create individual transcript line component.

**Tasks:**
- [ ] Create `TranscriptLine` component
- [ ] Display timestamp (optional)
- [ ] Show confidence indicator
- [ ] Add word-level highlighting

**Acceptance Criteria:**
- Lines render correctly
- Timestamps are formatted
- Confidence is visible

---

#### Issue #18: Partial Text Component
**Branch:** `feature/partial-text`
**Labels:** `frontend`, `ui`, `priority:high`

**Description:**
Create component for in-progress transcription.

**Tasks:**
- [ ] Create `PartialText` component
- [ ] Add typing animation
- [ ] Style as "in-progress"
- [ ] Handle rapid updates

**Acceptance Criteria:**
- Partial text is visually distinct
- Updates are smooth

---

#### Issue #19: Audio Controls Component
**Branch:** `feature/audio-controls`
**Labels:** `frontend`, `ui`, `priority:high`

**Description:**
Create recording control buttons.

**Tasks:**
- [ ] Create `AudioControls` component
- [ ] Add Start/Stop button
- [ ] Add Pause/Resume button
- [ ] Add keyboard shortcuts
- [ ] Show recording duration

**Acceptance Criteria:**
- Controls work correctly
- Keyboard shortcuts function
- Duration updates in real-time

---

#### Issue #20: Device Selector Component
**Branch:** `feature/device-selector`
**Labels:** `frontend`, `ui`, `priority:medium`

**Description:**
Create microphone device selection dropdown.

**Tasks:**
- [ ] Create `DeviceSelector` component
- [ ] List available devices
- [ ] Show current selection
- [ ] Handle device changes

**Acceptance Criteria:**
- All devices are listed
- Selection persists

---

#### Issue #21: Connection Status Component
**Branch:** `feature/connection-status-ui`
**Labels:** `frontend`, `ui`, `priority:medium`

**Description:**
Create connection status indicator.

**Tasks:**
- [ ] Create `ConnectionStatus` component
- [ ] Show connection state with icons
- [ ] Display reconnection attempts
- [ ] Add tooltip with details

**Acceptance Criteria:**
- Status is always visible
- States are clearly indicated

---

#### Issue #22: Server Configuration Component
**Branch:** `feature/server-config-ui`
**Labels:** `frontend`, `ui`, `priority:medium`

**Description:**
Create server configuration dialog.

**Tasks:**
- [ ] Create `ServerConfig` component
- [ ] Add URL input with validation
- [ ] Add API key input (masked)
- [ ] Add auth ID input
- [ ] Save to localStorage

**Acceptance Criteria:**
- Configuration persists
- Validation works
- Sensitive data is masked

---

#### Issue #23: VAD Indicator Component
**Branch:** `feature/vad-indicator`
**Labels:** `frontend`, `ui`, `priority:low`

**Description:**
Create voice activity detection indicator.

**Tasks:**
- [ ] Create `VadIndicator` component
- [ ] Show speech/silence state
- [ ] Add visual animation
- [ ] Display VAD confidence

**Acceptance Criteria:**
- VAD state is visible
- Transitions are smooth

---

#### Issue #24: Settings Page
**Branch:** `feature/settings-page`
**Labels:** `frontend`, `ui`, `priority:low`

**Description:**
Create dedicated settings page.

**Tasks:**
- [ ] Create settings page route
- [ ] Add audio settings section
- [ ] Add connection settings section
- [ ] Add display settings section
- [ ] Add export/import config

**Acceptance Criteria:**
- All settings are accessible
- Changes persist

---

#### Issue #25: Transcript History Page
**Branch:** `feature/history-page`
**Labels:** `frontend`, `ui`, `priority:low`

**Description:**
Create transcript history page.

**Tasks:**
- [ ] Create history page route
- [ ] List saved transcripts
- [ ] Add search/filter
- [ ] Add export functionality
- [ ] Add delete functionality

**Acceptance Criteria:**
- History is accessible
- Export works

---

### Testing & Quality

#### Issue #26: Unit Test Setup
**Branch:** `feature/unit-test-setup`
**Labels:** `testing`, `priority:high`

**Description:**
Set up Vitest for unit testing.

**Tasks:**
- [ ] Install and configure Vitest
- [ ] Add test utilities
- [ ] Create mock factories
- [ ] Add coverage reporting

**Acceptance Criteria:**
- `pnpm test` runs tests
- Coverage reports generate

---

#### Issue #27: Audio Pipeline Tests
**Branch:** `feature/audio-tests`
**Labels:** `testing`, `audio`, `priority:high`

**Description:**
Write tests for audio processing components.

**Tasks:**
- [ ] Test resampler accuracy
- [ ] Test chunk encoding
- [ ] Test level calculations
- [ ] Mock AudioContext for tests

**Acceptance Criteria:**
- All audio functions tested
- Edge cases covered

---

#### Issue #28: WebSocket Tests
**Branch:** `feature/websocket-tests`
**Labels:** `testing`, `websocket`, `priority:high`

**Description:**
Write tests for WebSocket client.

**Tasks:**
- [ ] Create mock WebSocket server
- [ ] Test connection lifecycle
- [ ] Test message handling
- [ ] Test reconnection logic

**Acceptance Criteria:**
- All WebSocket scenarios tested
- Mock server works reliably

---

#### Issue #29: E2E Test Setup
**Branch:** `feature/e2e-setup`
**Labels:** `testing`, `e2e`, `priority:medium`

**Description:**
Set up Playwright for E2E testing.

**Tasks:**
- [ ] Install and configure Playwright
- [ ] Create test fixtures
- [ ] Add browser matrix
- [ ] Set up CI integration

**Acceptance Criteria:**
- E2E tests run in CI
- Multiple browsers tested

---

#### Issue #30: E2E Transcription Tests
**Branch:** `feature/e2e-transcription`
**Labels:** `testing`, `e2e`, `priority:medium`

**Description:**
Write E2E tests for transcription flow.

**Tasks:**
- [ ] Test connection flow
- [ ] Test recording flow
- [ ] Test transcript display
- [ ] Test error scenarios

**Acceptance Criteria:**
- Full user flow tested
- Error cases covered

---

### Deployment & Documentation

#### Issue #31: Docker Configuration
**Branch:** `feature/docker-config`
**Labels:** `deployment`, `priority:medium`

**Description:**
Create Docker configuration for deployment.

**Tasks:**
- [ ] Create multi-stage Dockerfile
- [ ] Create docker-compose.yml
- [ ] Add health check endpoint
- [ ] Optimize image size

**Acceptance Criteria:**
- Docker build succeeds
- Container runs correctly

---

#### Issue #32: Vercel Configuration
**Branch:** `feature/vercel-config`
**Labels:** `deployment`, `priority:medium`

**Description:**
Configure Vercel deployment.

**Tasks:**
- [ ] Create vercel.json
- [ ] Configure environment variables
- [ ] Set up preview deployments
- [ ] Add security headers

**Acceptance Criteria:**
- Vercel deployment works
- Preview URLs function

---

#### Issue #33: CI/CD Pipeline
**Branch:** `feature/ci-cd`
**Labels:** `deployment`, `ci`, `priority:high`

**Description:**
Set up GitHub Actions CI/CD.

**Tasks:**
- [ ] Create lint workflow
- [ ] Create test workflow
- [ ] Create build workflow
- [ ] Create deploy workflow
- [ ] Add status badges

**Acceptance Criteria:**
- All workflows pass
- Deploy on merge to main

---

#### Issue #34: User Documentation
**Branch:** `feature/user-docs`
**Labels:** `documentation`, `priority:medium`

**Description:**
Write user-facing documentation.

**Tasks:**
- [ ] Create README.md
- [ ] Write getting started guide
- [ ] Document configuration options
- [ ] Add troubleshooting guide
- [ ] Create FAQ

**Acceptance Criteria:**
- Docs are complete
- Examples work

---

#### Issue #35: Developer Documentation
**Branch:** `feature/dev-docs`
**Labels:** `documentation`, `priority:low`

**Description:**
Write developer documentation.

**Tasks:**
- [ ] Document architecture
- [ ] Document component API
- [ ] Document testing approach
- [ ] Add contribution guide

**Acceptance Criteria:**
- Architecture is documented
- API is documented

---

## Implementation Timeline

### Phase 1: Foundation (Week 1-2)
- Issues #1-5: Project setup and configuration
- Issues #6-7: Core audio capture

### Phase 2: Core Features (Week 3-4)
- Issues #8-10: Complete audio pipeline
- Issues #11-15: WebSocket integration

### Phase 3: UI Development (Week 5-6)
- Issues #16-20: Core UI components
- Issues #21-25: Additional UI features

### Phase 4: Quality & Polish (Week 7-8)
- Issues #26-30: Testing
- Issues #31-35: Deployment and documentation

---

## Existing Moshi Client Implementation Reference

This section documents the actual implementation in `moshi/client/` which serves as the reference for the Next.js web client design.

### Source Code Structure

```
moshi/client/src/
├── app.tsx                          # React entry point with router
├── audio-processor.ts               # AudioWorklet for playback buffering
├── env.ts                           # Environment configuration
├── index.css                        # Global styles
├── modules.d.ts                     # TypeScript module declarations
├── components/
│   └── Button/                      # Reusable button component
├── decoder/
│   └── decoderWorker.ts             # Opus decoder worker loader
├── hooks/
│   └── useAuth.ts                   # Better Auth authentication hook
├── lib/
│   └── auth-client.ts               # Better Auth client configuration
├── pages/
│   ├── Conversation/                # Main conversation UI
│   │   ├── Conversation.tsx         # Main conversation component
│   │   ├── MediaContext.ts          # Audio context provider
│   │   ├── SocketContext.ts         # WebSocket context provider
│   │   ├── getMimeType.ts           # MIME type detection
│   │   ├── components/              # UI components
│   │   │   ├── AudioVisualizer/     # Audio visualization
│   │   │   ├── ServerAudio/         # Server audio playback
│   │   │   ├── UserAudio/           # User microphone capture
│   │   │   ├── TextDisplay/         # Transcript display
│   │   │   ├── ServerInfo/          # Server metadata display
│   │   │   └── ModelParams/         # Model parameter controls
│   │   └── hooks/                   # Conversation-specific hooks
│   │       ├── useSocket.ts         # WebSocket connection management
│   │       ├── useServerAudio.ts    # Server audio decoding/playback
│   │       ├── useUserAudio.ts      # Microphone capture with opus-recorder
│   │       ├── useServerText.ts     # Transcript text handling
│   │       ├── useServerInfo.ts     # Server metadata parsing
│   │       └── useModelParams.ts    # Model parameter state
│   └── Queue/                       # Landing/queue page
│       └── Queue.tsx                # Entry point with settings
└── protocol/
    ├── types.ts                     # Message type definitions
    ├── encoder.ts                   # Binary protocol encoder/decoder
    └── testMessages.ts              # Test message utilities
```

### Binary Protocol Specification

The moshi server uses a **custom binary protocol** (not MessagePack). Each message starts with a 1-byte type identifier:

| Type Byte | Message Type | Payload |
|-----------|--------------|---------|
| `0x00` | Handshake | `[version: u8, model: u8]` |
| `0x01` | Audio | `[opus_data: bytes]` |
| `0x02` | Text | `[utf8_text: bytes]` |
| `0x03` | Control | `[action: u8]` (0=start, 1=endTurn, 2=pause, 3=restart) |
| `0x04` | Metadata | `[json_utf8: bytes]` |
| `0x05` | Error | `[utf8_message: bytes]` |
| `0x06` | Ping | (no payload) |
| `0x07` | ColoredText | `[color: u8, utf8_text: bytes]` |

**Source:** `moshi/client/src/protocol/encoder.ts`

### Audio Pipeline Details

#### Microphone Capture (useUserAudio)

The `useUserAudio` hook manages microphone capture with opus-recorder:

```typescript
// Key configuration from moshi/client/src/pages/Conversation/hooks/useUserAudio.ts
const recorderOptions = {
  encoderPath,                        // opus-recorder/dist/encoderWorker.min.js
  bufferLength: 960 * sampleRate / 24000,  // Scaled buffer
  encoderFrameSize: 20,               // 20ms Opus frames
  encoderSampleRate: 24000,           // Target: 24kHz
  maxFramesPerPage: 2,                // Low latency: 2 frames/page
  numberOfChannels: 1,                // Mono
  recordingGain: 1,                   // No gain adjustment
  resampleQuality: 3,                 // Quality 0-10
  encoderComplexity: 0,               // Low complexity for speed
  encoderApplication: 2049,           // OPUS_APPLICATION_VOIP
  streamPages: true,                  // Stream as ready
};
```

**Audio Constraints:**
```typescript
{
  audio: {
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
    channelCount: 1,
  },
  video: false,
}
```

#### Audio Playback (MoshiProcessor AudioWorklet)

The `audio-processor.ts` implements an AudioWorklet for buffered playback:

**Key Parameters:**
- **frameSize:** 80ms at sample rate
- **initialBufferSamples:** 1 × frameSize (wait before playback)
- **partialBufferSamples:** 10ms (additional delay)
- **maxBufferSamples:** 10ms (drop threshold)

**Buffer Management:**
1. Frames are queued via `port.postMessage({ frame, type: 'audio', micDuration })`
2. Playback starts when buffer reaches `initialBufferSamples`
3. If buffer exceeds `maxBufferSamples`, oldest packets are dropped
4. Fade-in applied on first output, fade-out on buffer underrun

**Metrics Reported:**
```typescript
{
  totalAudioPlayed: number,    // Total time including silence
  actualAudioPlayed: number,   // Actual audio output time
  delay: number,               // Current delay
  minDelay: number,            // Minimum observed delay
  maxDelay: number,            // Maximum observed delay
}
```

#### Opus Decoding (decoderWorker)

The decoder worker is loaded from pre-built assets:
```typescript
// moshi/client/src/decoder/decoderWorker.ts
export const DecoderWorker = new Worker(
  new URL("/assets/decoderWorker.min.js", import.meta.url),
);
```

**Initialization:**
```typescript
decoderWorker.postMessage({
  command: "init",
  bufferLength: 960 * audioContext.sampleRate / 24000,
  decoderSampleRate: 24000,
  outputBufferSampleRate: audioContext.sampleRate,
  resampleQuality: 0,
});
```

### WebSocket Connection (useSocket)

The `useSocket` hook manages WebSocket lifecycle:

**Key Features:**
- Binary message type: `arraybuffer`
- Handshake-based connection confirmation
- 10-second inactivity timeout
- Automatic disconnect on timeout

**Connection Flow:**
1. Create WebSocket with `binaryType = 'arraybuffer'`
2. Wait for `0x00` (handshake) message from server
3. Set `isConnected = true` after handshake
4. Start inactivity timer (10s)
5. Close socket if no messages received

**URL Building (Conversation.tsx):**
```typescript
const buildURL = ({ workerAddr, params, workerAuthId, sessionToken, email, textSeed, audioSeed }) => {
  const wsProtocol = window.location.protocol === "https:" ? "wss" : "ws";
  const url = new URL(`${wsProtocol}://${workerAddr}/api/chat`);
  
  // Authentication
  if (workerAuthId) url.searchParams.append("worker_auth_id", workerAuthId);
  if (sessionToken) url.searchParams.append("auth_id", sessionToken);
  if (email) url.searchParams.append("email", email);
  
  // Model parameters
  url.searchParams.append("text_temperature", params.textTemperature.toString());
  url.searchParams.append("text_topk", params.textTopk.toString());
  url.searchParams.append("audio_temperature", params.audioTemperature.toString());
  url.searchParams.append("audio_topk", params.audioTopk.toString());
  url.searchParams.append("pad_mult", params.padMult.toString());
  url.searchParams.append("text_seed", textSeed.toString());
  url.searchParams.append("audio_seed", audioSeed.toString());
  url.searchParams.append("repetition_penalty_context", params.repetitionPenaltyContext.toString());
  url.searchParams.append("repetition_penalty", params.repetitionPenalty.toString());
  
  // Optional image mode
  if (params.imageUrl) {
    url.searchParams.append("image_url", params.imageUrl.toString());
    url.searchParams.append("image_resolution", params.imageResolution.toString());
  }
  
  return url.toString();
};
```

### Authentication Integration

#### Better Auth Client (lib/auth-client.ts)

```typescript
import { createAuthClient } from "better-auth/react";

export const authClient = createAuthClient({
  baseURL: import.meta.env.VITE_AUTH_URL || undefined,
});

export const { signIn, signUp, signOut, useSession, getSession } = authClient;

export async function getSessionToken(): Promise<string | null> {
  const session = await getSession();
  if (!session.data) return null;
  return session.data.session.token;
}
```

#### useAuth Hook (hooks/useAuth.ts)

```typescript
export interface AuthState {
  user: User | null;
  session: Session | null;
  isLoading: boolean;
  isAuthenticated: boolean;
}

export function useAuth(): AuthState & {
  signIn: typeof authClient.signIn;
  signUp: typeof authClient.signUp;
  signOut: () => Promise<void>;
  getToken: () => Promise<string | null>;
}
```

### Model Parameters (useModelParams)

Default values and ranges:

| Parameter | Default | Range |
|-----------|---------|-------|
| textTemperature | 0.7 | 0.2 - 1.2 |
| textTopk | 25 | 10 - 500 |
| audioTemperature | 0.8 | 0.2 - 1.2 |
| audioTopk | 250 | 10 - 500 |
| padMult | 0 | -4 - 4 |
| repetitionPenalty | 1.0 | 1.0 - 2.0 |
| repetitionPenaltyContext | 64 | 0 - 200 |
| imageResolution | 224 | 64 - 512 |

### Server Metadata Schema (useServerInfo)

The server sends metadata via `0x04` message type:

```typescript
const ServersInfoSchema = z.object({
  text_temperature: z.number(),
  text_topk: z.number(),
  audio_temperature: z.number(),
  audio_topk: z.number(),
  pad_mult: z.number(),
  repetition_penalty_context: z.number(),
  repetition_penalty: z.number(),
  lm_model_file: z.string(),
  instance_name: z.string(),
  build_info: z.object({
    build_timestamp: z.string(),
    build_date: z.string(),
    git_branch: z.string(),
    git_timestamp: z.string(),
    git_date: z.string(),
    git_hash: z.string(),
    git_describe: z.string(),
    rustc_host_triple: z.string(),
    rustc_version: z.string(),
    cargo_target_triple: z.string(),
  }),
});
```

### Text Display with Color Support

The `TextDisplay` component supports colored text via `0x07` (coloredtext) messages:

```typescript
// Color palette (purple to green)
const textDisplayColors = [
  "#d19bf7", "#d7acf6", "#debdf5", "#e4cef4",
  "#ebe0f3", "#eef2f0", "#c8ead9", "#a4e2c4",
  "#80d9af", "#5bd09a", "#38c886"
];
```

### Recording & Export

The `Conversation` component supports recording both audio and video:

**Audio Recording:**
- Uses `MediaRecorder` with `audioStreamDestination`
- MIME type detection via `getMimeType('audio')`
- Supports: `audio/webm`, `audio/mpeg`, `audio/mp4`
- WebM duration fix applied via `webm-duration-fix`

**Video Recording:**
- Canvas capture at 30fps with audio track
- MIME type detection via `getMimeType('video')`
- Supports: `video/webm`, `video/mp4`

### Context Providers

#### MediaContext
```typescript
type MediaContextType = {
  startRecording: () => void;
  stopRecording: () => void;
  audioContext: MutableRefObject<AudioContext>;
  audioStreamDestination: MutableRefObject<MediaStreamAudioDestinationNode>;
  worklet: MutableRefObject<AudioWorkletNode>;
  micDuration: MutableRefObject<number>;
  actualAudioPlayed: MutableRefObject<number>;
};
```

#### SocketContext
```typescript
type SocketContextType = {
  isConnected: boolean;
  socket: WebSocket | null;
  sendMessage: (message: WSMessage) => void;
};
```

### Required Assets

Copy from `moshi/client/public/assets/` to new project:
- `decoderWorker.min.js` - Opus decoder worker
- `decoderWorker.min.wasm` - Opus decoder WASM binary

### Dependencies

From `moshi/client/package.json`:
- `opus-recorder` - Opus encoding
- `webm-duration-fix` - Fix WebM duration metadata
- `better-auth` - Authentication
- `zod` - Schema validation
- `eruda` - Mobile debugging (dev only)
- `react-router-dom` - Routing

---

## Appendix

### A. Browser Compatibility Matrix

| Feature | Chrome | Firefox | Safari | Edge |
|---------|--------|---------|--------|------|
| AudioWorklet | 66+ | 76+ | 14.1+ | 79+ |
| getUserMedia | 53+ | 36+ | 11+ | 12+ |
| WebSocket | 16+ | 11+ | 6+ | 12+ |
| WebAssembly | 57+ | 52+ | 11+ | 16+ |
| MessagePack | ✓ | ✓ | ✓ | ✓ |

### B. Performance Targets

| Metric | Target |
|--------|--------|
| Audio latency | < 100ms |
| Transcript latency | < 500ms |
| Memory usage | < 100MB |
| CPU usage | < 10% |
| Bundle size | < 200KB gzipped |

### C. Accessibility Requirements

- WCAG 2.1 AA compliance
- Keyboard navigation for all controls
- Screen reader support for transcripts
- High contrast mode support
- Reduced motion support

### D. Document Changelog

| Version | Date | Changes |
|---------|------|---------|
| 2.0.0 | Dec 2024 | Added Agent Implementation Guide, Quick Reference, file creation order, pitfalls table |
| 1.2.0 | Dec 2024 | Added Moshi client reference, Better Auth integration |
| 1.1.0 | Dec 2024 | Added UI/UX design, component specifications |
| 1.0.0 | Dec 2024 | Initial systems design document |

---

*Document optimized for AI coding agents. Version 2.0.0 - December 2024.*
