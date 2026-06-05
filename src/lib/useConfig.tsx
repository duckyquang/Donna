import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { api, type AppConfig } from "./api";

interface ConfigContextValue {
  config: AppConfig | null;
  loading: boolean;
  refresh: () => Promise<void>;
  save: (config: AppConfig) => Promise<void>;
}

const ConfigContext = createContext<ConfigContextValue | null>(null);

export function ConfigProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
    const c = await api.getConfig();
    setConfig(c);
  };

  const save = async (next: AppConfig) => {
    await api.saveConfig(next);
    setConfig(next);
  };

  useEffect(() => {
    refresh()
      .catch(() => setConfig(null))
      .finally(() => setLoading(false));
  }, []);

  return (
    <ConfigContext.Provider value={{ config, loading, refresh, save }}>
      {children}
    </ConfigContext.Provider>
  );
}

export function useConfig(): ConfigContextValue {
  const ctx = useContext(ConfigContext);
  if (!ctx) throw new Error("useConfig must be used within a ConfigProvider");
  return ctx;
}
