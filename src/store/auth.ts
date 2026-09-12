import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { User } from "../lib/tauri";

interface AuthState {
  user: User | null;
  setUser: (user: User | null) => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      setUser: (user) => set({ user }),
    }),
    { name: "soffit-auth" }
  )
);
