import React, { useState } from "react";
import { ArrowUp, Bot, FileText } from "lucide-react";
import { aiChatDocument } from "../../lib/tauri";

type Message = { role: "user" | "assistant"; text: string };

export function DocumentChat({ filePath, fileKind, fileName, documentText }: { filePath: string; fileKind: string; fileName: string; documentText?: string }) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [question, setQuestion] = useState("");
  const [loading, setLoading] = useState(false);

  const send = async () => {
    const prompt = question.trim();
    if (!prompt || loading) return;
    setQuestion(""); setLoading(true);
    setMessages(items => [...items, { role: "user", text: prompt }]);
    try {
      const answer = await aiChatDocument(filePath, fileKind, prompt, documentText);
      setMessages(items => [...items, { role: "assistant", text: answer.text }]);
    } catch (error) {
      setMessages(items => [...items, { role: "assistant", text: `I couldn't read this document: ${error}` }]);
    } finally { setLoading(false); }
  };

  return <aside style={{ width: 330, minWidth: 280, display: "flex", flexDirection: "column", borderLeft: "1px solid var(--color-border)", background: "var(--color-surface)" }}>
    <div style={{ padding: "14px 16px", borderBottom: "1px solid var(--color-border)" }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center", fontWeight: 600, fontSize: 13 }}><Bot size={16} color="var(--color-primary)" /> Ask this document</div>
      <p style={{ marginTop: 5, color: "var(--color-text-muted)", fontSize: 11, display: "flex", alignItems: "center", gap: 4 }}><FileText size={11} /> {fileName}</p>
    </div>
    <div style={{ flex: 1, overflowY: "auto", padding: 14, display: "flex", flexDirection: "column", gap: 10 }}>
      {messages.length === 0 && <p style={{ color: "var(--color-text-secondary)", fontSize: 12, lineHeight: 1.5 }}>Ask for a summary, key figures, or an explanation of what’s in this local document.</p>}
      {messages.map((message, index) => <div key={index} style={{ alignSelf: message.role === "user" ? "flex-end" : "flex-start", maxWidth: "92%", padding: "9px 10px", borderRadius: 9, background: message.role === "user" ? "var(--color-primary)" : "var(--color-bg)", color: message.role === "user" ? "white" : "var(--color-ink)", fontSize: 12, lineHeight: 1.45, whiteSpace: "pre-wrap" }}>{message.text}</div>)}
      {loading && <p style={{ color: "var(--color-text-muted)", fontSize: 12 }}>Reading locally…</p>}
    </div>
    <div style={{ padding: 12, borderTop: "1px solid var(--color-border)", display: "flex", gap: 7 }}>
      <textarea aria-label="Ask this document" value={question} onChange={event => setQuestion(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); send(); } }} placeholder="Ask about this document…" rows={2} style={{ flex: 1, resize: "none", border: "1px solid var(--color-border)", borderRadius: 7, padding: "7px 8px", background: "var(--color-bg)", color: "var(--color-ink)", outline: "none", fontSize: 12 }} />
      <button aria-label="Send question" onClick={send} disabled={!question.trim() || loading} style={{ alignSelf: "flex-end", border: "none", borderRadius: 7, height: 30, width: 30, background: "var(--color-primary)", color: "white", opacity: !question.trim() || loading ? .45 : 1 }}><ArrowUp size={15} /></button>
    </div>
  </aside>;
}
