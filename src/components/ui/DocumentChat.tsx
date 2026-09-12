import React, { useEffect, useRef, useState } from "react";
import { ArrowUp, Bot, FileText, Sparkles } from "lucide-react";
import { aiChatDocument } from "../../lib/tauri";
import { listen } from "@tauri-apps/api/event";

type Message = { role: "user" | "assistant"; text: string };

export function DocumentChat({ filePath, fileKind, fileName, documentText }: { filePath: string; fileKind: string; fileName: string; documentText?: string }) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [question, setQuestion] = useState("");
  const [loading, setLoading] = useState(false);
  const [streamedText, setStreamedText] = useState("");
  const transcriptRef = useRef<HTMLDivElement>(null);
  useEffect(() => { transcriptRef.current?.scrollTo({ top: transcriptRef.current.scrollHeight, behavior: "smooth" }); }, [messages, loading]);

  const send = async () => {
    const prompt = question.trim();
    if (!prompt || loading) return;
    setQuestion(""); setLoading(true); setStreamedText("");
    setMessages(items => [...items, { role: "user", text: prompt }]);
    let unlisten: (() => void) | undefined;
    let receivedTokens = false;
    try {
      // The backend emits each local token while generation runs. Set up the
      // listener before invoking so the first visible response is immediate.
      unlisten = await listen<string>("ai-token", event => {
        receivedTokens = true;
        setStreamedText(text => text + event.payload);
      });
      // Short follow-ups such as "Gimme more" need the preceding answer.
      // Keep only a small recent window so conversational context never turns
      // into another long-prompt performance problem.
      const history = messages.slice(-4).map(message =>
        `${message.role === "user" ? "User" : "Assistant"}: ${message.text}`
      ).join("\n");
      const request = history
        ? `Recent conversation:\n${history}\n\nCurrent user question: ${prompt}`
        : prompt;
      const answer = await aiChatDocument(filePath, fileKind, request, documentText);
      setMessages(items => [...items, { role: "assistant", text: answer.text }]);
    } catch (error) {
      setMessages(items => [...items, { role: "assistant", text: `I couldn't read this document: ${error}` }]);
    } finally {
      unlisten?.();
      if (!receivedTokens) setStreamedText("");
      setLoading(false);
    }
  };

  return <aside style={{ width: 360, minWidth: 310, display: "flex", flexDirection: "column", borderLeft: "1px solid var(--color-border)", background: "var(--color-surface)" }}>
    <div style={{ padding: "18px 18px 14px", borderBottom: "1px solid var(--color-border)" }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center", fontWeight: 700, fontSize: 14 }}><Bot size={17} color="var(--color-primary)" /> Ask this document</div>
      <p style={{ marginTop: 6, color: "var(--color-text-muted)", fontSize: 11, display: "flex", alignItems: "center", gap: 5, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}><FileText size={12} /> {fileName}</p>
    </div>
    <div ref={transcriptRef} style={{ flex: 1, overflowY: "auto", padding: 18, display: "flex", flexDirection: "column", gap: 12 }}>
      {messages.length === 0 && <div style={{ padding: 14, background: "var(--color-bg)", border: "1px solid var(--color-border)", fontSize: 12, lineHeight: 1.55, color: "var(--color-text-secondary)" }}><Sparkles size={16} color="var(--color-primary)" /><p style={{ marginTop: 8 }}>Ask for a summary, key figures, or a plain-language explanation. Your document stays on this device.</p><div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 12 }}>{["Summarise this", "What are the key figures?", "Explain this simply"].map(prompt => <button key={prompt} onClick={() => setQuestion(prompt)} style={{ border: "1px solid var(--color-border)", background: "var(--color-surface)", color: "var(--color-ink)", padding: "5px 7px", borderRadius: 5, fontSize: 11 }}>{prompt}</button>)}</div></div>}
      {messages.map((message, index) => <div key={index} style={{ alignSelf: message.role === "user" ? "flex-end" : "flex-start", maxWidth: "94%", padding: "10px 11px", borderRadius: message.role === "user" ? "9px 2px 9px 9px" : "2px 9px 9px 9px", background: message.role === "user" ? "var(--color-primary)" : "var(--color-bg)", color: message.role === "user" ? "white" : "var(--color-ink)", fontSize: 12, lineHeight: 1.55, whiteSpace: "pre-wrap" }}>{message.text}</div>)}
      {loading && streamedText && <div style={{ alignSelf: "flex-start", maxWidth: "94%", padding: "10px 11px", borderRadius: "2px 9px 9px 9px", background: "var(--color-bg)", color: "var(--color-ink)", fontSize: 12, lineHeight: 1.55, whiteSpace: "pre-wrap" }}>{streamedText}</div>}
      {loading && <p style={{ color: "var(--color-text-muted)", fontSize: 12 }}>Reading document locally…</p>}
    </div>
    <div style={{ padding: 14, borderTop: "1px solid var(--color-border)", display: "flex", gap: 8 }}>
      <textarea aria-label="Ask this document" value={question} onChange={event => setQuestion(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); send(); } }} placeholder="Ask about this document…" rows={2} style={{ flex: 1, resize: "none", border: "1px solid var(--color-border)", borderRadius: 7, padding: "7px 8px", background: "var(--color-bg)", color: "var(--color-ink)", outline: "none", fontSize: 12 }} />
      <button aria-label="Send question" onClick={send} disabled={!question.trim() || loading} style={{ alignSelf: "flex-end", border: "none", borderRadius: 7, height: 30, width: 30, background: "var(--color-primary)", color: "white", opacity: !question.trim() || loading ? .45 : 1 }}><ArrowUp size={15} /></button>
    </div>
  </aside>;
}
