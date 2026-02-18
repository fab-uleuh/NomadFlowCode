import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Moon, Sun } from 'lucide-react-native';
import { useStorage } from '@/lib/context/storage-context';

type AiAgent = 'claude' | 'ollama' | 'custom';

interface SettingsDialogProps {
  colorScheme: 'light' | 'dark';
  onToggleTheme: () => void;
  onClose: () => void;
}

export function SettingsDialog({
  colorScheme,
  onToggleTheme,
  onClose,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const { settings, updateSettings, clearAllData } = useStorage();

  const AI_AGENTS: { value: AiAgent; label: string; description: string }[] = [
    { value: 'claude', label: t('settings.ai_agent.claude_label'), description: t('settings.ai_agent.claude_description') },
    { value: 'ollama', label: t('settings.ai_agent.ollama_label'), description: t('settings.ai_agent.ollama_description') },
    { value: 'custom', label: t('settings.ai_agent.custom_label'), description: t('settings.ai_agent.custom_description') },
  ];

  const [customCommand, setCustomCommand] = useState(settings.customAgentCommand || '');
  const [fontSize, setFontSize] = useState(settings.fontSize.toString());
  const [tmuxPrefix, setTmuxPrefix] = useState(settings.tmuxSessionPrefix);

  const handleSave = async () => {
    await updateSettings({
      customAgentCommand: customCommand,
      fontSize: parseInt(fontSize) || 14,
      tmuxSessionPrefix: tmuxPrefix,
    });
    onClose();
  };

  const handleClearData = async () => {
    if (window.confirm(t('settings.clear_data.confirm_web'))) {
      await clearAllData();
      onClose();
    }
  };

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-background rounded-xl border border-border p-6 w-[480px] max-w-[90vw] max-h-[80vh] overflow-y-auto shadow-[0_8px_32px_rgba(0,0,0,0.25)]">
        <h2 className="text-lg font-semibold mb-5">{t('settings.title')}</h2>

        <div className="flex flex-col gap-4">
          {/* Theme */}
          <div className="p-4 rounded-[10px] border border-border bg-card">
            <h3 className="flex items-center gap-1.5 text-sm font-semibold mb-3">
              {colorScheme === 'dark' ? <Moon size={14} /> : <Sun size={14} />}
              <span>{t('settings.appearance.title')}</span>
            </h3>
            <button
              onClick={onToggleTheme}
              className="flex items-center justify-between w-full px-3.5 py-2.5 border-none rounded-lg bg-muted text-foreground text-sm cursor-pointer">
              <span>{t('settings.appearance.theme')}</span>
              <span className="text-muted-foreground capitalize">
                {t(`settings.theme.${colorScheme}`)}
              </span>
            </button>
          </div>

          {/* AI Agent */}
          <div className="p-4 rounded-[10px] border border-border bg-card">
            <h3 className="text-sm font-semibold mb-3">{t('settings.ai_agent.title')}</h3>
            <div className="flex flex-col gap-2">
              {AI_AGENTS.map((agent) => (
                <button
                  key={agent.value}
                  onClick={() => updateSettings({ defaultAiAgent: agent.value })}
                  className={`flex items-center justify-between px-3.5 py-2.5 rounded-lg text-foreground text-sm cursor-pointer text-left ${
                    settings.defaultAiAgent === agent.value
                      ? 'border border-primary bg-transparent'
                      : 'border-none bg-muted'
                  }`}>
                  <div>
                    <div className="font-medium">{agent.label}</div>
                    <div className="text-xs text-muted-foreground">
                      {agent.description}
                    </div>
                  </div>
                  {settings.defaultAiAgent === agent.value && (
                    <div className="w-4 h-4 rounded-full bg-primary" />
                  )}
                </button>
              ))}

              {settings.defaultAiAgent === 'custom' && (
                <div className="mt-2">
                  <label className="block text-[13px] font-medium mb-1 text-foreground">
                    {t('settings.ai_agent.custom_command')}
                  </label>
                  <input
                    type="text"
                    placeholder={t('settings.ai_agent.custom_placeholder')}
                    value={customCommand}
                    onChange={(e) => setCustomCommand(e.target.value)}
                    className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
                  />
                </div>
              )}

              <button
                onClick={() => updateSettings({ autoLaunchAgent: !settings.autoLaunchAgent })}
                className="flex items-center justify-between px-3.5 py-2.5 border-none rounded-lg bg-muted text-foreground text-sm cursor-pointer mt-1">
                <div>
                  <div className="font-medium text-left">{t('settings.ai_agent.auto_launch')}</div>
                  <div className="text-xs text-muted-foreground text-left">
                    {t('settings.ai_agent.auto_launch_hint')}
                  </div>
                </div>
                <div
                  className={`w-10 h-[22px] rounded-[11px] relative transition-colors duration-200 ${
                    settings.autoLaunchAgent ? 'bg-primary' : 'bg-border'
                  }`}>
                  <div
                    className={`w-[18px] h-[18px] rounded-full bg-white absolute top-0.5 transition-[left] duration-200 ${
                      settings.autoLaunchAgent ? 'left-5' : 'left-0.5'
                    }`}
                  />
                </div>
              </button>
            </div>
          </div>

          {/* Terminal */}
          <div className="p-4 rounded-[10px] border border-border bg-card">
            <h3 className="text-sm font-semibold mb-3">{t('settings.terminal.title')}</h3>
            <div className="flex flex-col gap-3">
              <div>
                <label className="block text-[13px] font-medium mb-1 text-foreground">
                  {t('settings.terminal.font_size')}
                </label>
                <input
                  type="number"
                  value={fontSize}
                  onChange={(e) => setFontSize(e.target.value)}
                  className="w-[100px] px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
                />
              </div>
              <div>
                <label className="block text-[13px] font-medium mb-1 text-foreground">
                  {t('settings.terminal.tmux_prefix')}
                </label>
                <input
                  type="text"
                  value={tmuxPrefix}
                  onChange={(e) => setTmuxPrefix(e.target.value)}
                  className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
                />
              </div>
              <button
                onClick={() => updateSettings({ autoReconnect: !settings.autoReconnect })}
                className="flex items-center justify-between px-3.5 py-2.5 border-none rounded-lg bg-muted text-foreground text-sm cursor-pointer">
                <div>
                  <div className="font-medium text-left">{t('settings.terminal.auto_reconnect')}</div>
                  <div className="text-xs text-muted-foreground text-left">
                    {t('settings.terminal.auto_reconnect_hint')}
                  </div>
                </div>
                <div
                  className={`w-10 h-[22px] rounded-[11px] relative transition-colors duration-200 ${
                    settings.autoReconnect ? 'bg-primary' : 'bg-border'
                  }`}>
                  <div
                    className={`w-[18px] h-[18px] rounded-full bg-white absolute top-0.5 transition-[left] duration-200 ${
                      settings.autoReconnect ? 'left-5' : 'left-0.5'
                    }`}
                  />
                </div>
              </button>
            </div>
          </div>

          {/* Danger zone */}
          <div className="p-4 rounded-[10px] border border-destructive bg-card">
            <h3 className="text-sm font-semibold mb-3 text-destructive">
              {t('settings.danger_zone.title')}
            </h3>
            <button
              onClick={handleClearData}
              className="px-4 py-2 border-none rounded-lg bg-destructive text-white text-sm cursor-pointer">
              {t('settings.danger_zone.clear_button')}
            </button>
          </div>

          {/* Actions */}
          <div className="flex gap-3 justify-end mt-1">
            <button
              onClick={onClose}
              className="px-4 py-2 border border-border rounded-lg bg-transparent text-foreground text-sm cursor-pointer">
              {t('common.cancel')}
            </button>
            <button
              onClick={handleSave}
              className="px-4 py-2 border-none rounded-lg bg-primary text-primary-foreground text-sm font-medium cursor-pointer">
              {t('common.save')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
