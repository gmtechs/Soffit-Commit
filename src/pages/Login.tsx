import React, { useState } from "react";
import { useNavigate } from "react-router-dom";
import { HardDrive, Eye, EyeOff } from "lucide-react";
import { login } from "../lib/tauri";
import { useAuthStore } from "../store/auth";
import { useToast } from "../components/ui/Toast";
import { Button } from "../components/ui/Button";

export function LoginPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [loading, setLoading] = useState(false);
  const { setUser } = useAuthStore();
  const { toast } = useToast();
  const navigate = useNavigate();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password.length < 8) { toast("danger", "Password must be at least 8 characters"); return; }
    setLoading(true);
    try {
      const user = await login(username, password);
      setUser(user);
      navigate("/");
    } catch (err: any) {
      toast("danger", String(err));
    } finally {
      setLoading(false);
    }
  };

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "10px 12px", borderRadius: 10, border: "1px solid var(--color-border)",
    fontSize: 14, outline: "none", background: "var(--color-bg)", color: "var(--color-ink)",
  };

  return (
    <div style={{ minHeight: "100vh", background: "var(--color-bg)", display: "flex", alignItems: "center", justifyContent: "center" }}>
      <div style={{ width: 380, background: "var(--color-surface)", borderRadius: 16, border: "1px solid var(--color-border)", padding: 32 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 24 }}>
          <div style={{ width: 36, height: 36, borderRadius: 8, background: "var(--color-primary)", display: "flex", alignItems: "center", justifyContent: "center" }}>
            <HardDrive size={20} color="white" />
          </div>
          <span style={{ fontSize: 18, fontWeight: 700 }}>Soffit Commit</span>
        </div>

        <h1 style={{ fontSize: 20, fontWeight: 700, marginBottom: 4 }}>
          Sign in
        </h1>
        <p style={{ fontSize: 13, color: "var(--color-text-secondary)", marginBottom: 24 }}>
          Enter the credentials issued by a user who already has access to this device.
        </p>

        <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          <div>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Username</label>
            <input style={inputStyle} value={username} onChange={(e) => setUsername(e.target.value)} required autoFocus placeholder="your-username" />
          </div>
          <div>
            <label style={{ fontSize: 12, fontWeight: 500, display: "block", marginBottom: 6 }}>Password</label>
            <div style={{ position: "relative" }}>
              <input style={{ ...inputStyle, paddingRight: 36 }} type={showPw ? "text" : "password"} value={password} onChange={(e) => setPassword(e.target.value)} required placeholder="••••••••" />
              <button type="button" onClick={() => setShowPw(!showPw)} style={{ position: "absolute", right: 10, top: "50%", transform: "translateY(-50%)", background: "none", border: "none", cursor: "pointer", color: "var(--color-text-muted)" }}>
                {showPw ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </div>
          <p style={{ fontSize: 11, color: "var(--color-text-muted)", padding: "8px 12px", background: "var(--color-bg)", borderRadius: 8 }}>
            New accounts are created from Settings by an existing user.
          </p>
          <Button type="submit" variant="primary" disabled={loading} style={{ width: "100%", marginTop: 4 }}>
            {loading ? "Please wait…" : "Sign in"}
          </Button>
        </form>
      </div>
    </div>
  );
}
