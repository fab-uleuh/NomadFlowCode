import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { executeServerCommand } from '@/lib/server-commands';
import type { Server, BranchInfo } from '@shared';

interface CreateFeatureDialogProps {
  server: Server;
  repoPath: string;
  onClose: () => void;
  onCreated: (featureName: string, worktreePath: string, branch: string) => void;
}

type Tab = 'new' | 'existing';

export function CreateFeatureDialog({
  server,
  repoPath,
  onClose,
  onCreated,
}: CreateFeatureDialogProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<Tab>('new');

  // New branch
  const [branchName, setBranchName] = useState('');
  const [baseBranch, setBaseBranch] = useState('main');

  // Shared
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [allBranches, setAllBranches] = useState<BranchInfo[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [isLoadingBranches, setIsLoadingBranches] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState('');

  const loadBranches = useCallback(async () => {
    setIsLoadingBranches(true);
    try {
      const result = await executeServerCommand(server, {
        action: 'list-branches',
        params: { repoPath },
      });
      if (result.success && result.data) {
        const defaultBr = result.data.defaultBranch || 'main';
        setBranches(result.data.branches);
        setBaseBranch(defaultBr);
        setAllBranches(result.data.branches);
      }
    } catch {
      // Silently fail
    } finally {
      setIsLoadingBranches(false);
    }
  }, [server, repoPath]);

  useEffect(() => {
    loadBranches();
  }, [loadBranches]);

  const createNewBranch = async () => {
    const trimmed = branchName.trim();
    if (!trimmed) {
      setError(t('features.create.error.branch_name_required'));
      return;
    }

    const sanitized = trimmed
      .replace(/[^a-zA-Z0-9-_/.]/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');

    if (!sanitized) {
      setError(t('features.create.error.invalid_branch_name'));
      return;
    }

    setIsCreating(true);
    setError('');

    try {
      const result = await executeServerCommand(server, {
        action: 'create-feature',
        params: { repoPath, branchName: sanitized, baseBranch: baseBranch || 'main' },
      });

      if (result.success && result.data) {
        const featureName = result.data.worktreePath.split('/').pop() || sanitized;
        onCreated(featureName, result.data.worktreePath, result.data.branch);
      } else {
        throw new Error(result.error || t('features.create.error.creation_failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('common.error.creation_failed'));
    } finally {
      setIsCreating(false);
    }
  };

  const attachExistingBranch = async () => {
    if (!selectedBranch) return;

    setIsCreating(true);
    setError('');

    try {
      const result = await executeServerCommand(server, {
        action: 'attach-branch',
        params: { repoPath, branchName: selectedBranch },
      });

      if (result.success && result.data) {
        const featureName = result.data.worktreePath.split('/').pop() || selectedBranch;
        onCreated(featureName, result.data.worktreePath, result.data.branch);
      } else {
        throw new Error(result.error || t('features.create.error.attachment_failed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('features.create.error.attachment_failed'));
    } finally {
      setIsCreating(false);
    }
  };

  const filteredBranches = branches.filter((b) =>
    b.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const filteredBaseBranches = allBranches.filter((b) =>
    b.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div
      onClick={onClose}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-[9999]">
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-card rounded-xl border border-border p-6 w-[460px] max-w-[90vw] max-h-[80vh] flex flex-col shadow-[0_8px_32px_rgba(0,0,0,0.25)]">
        <h2 className="text-lg font-semibold mb-4">{t('features.create.title')}</h2>

        {/* Tabs */}
        <div className="flex rounded-lg bg-muted p-1 mb-4">
          <button
            className={`flex-1 py-2 border-none rounded-md text-[13px] font-medium cursor-pointer text-center ${
              activeTab === 'new'
                ? 'bg-background text-foreground'
                : 'bg-transparent text-muted-foreground'
            }`}
            onClick={() => {
              setActiveTab('new');
              setSearchQuery('');
              setSelectedBranch(null);
            }}>
            {t('features.modal.tab.new')}
          </button>
          <button
            className={`flex-1 py-2 border-none rounded-md text-[13px] font-medium cursor-pointer text-center ${
              activeTab === 'existing'
                ? 'bg-background text-foreground'
                : 'bg-transparent text-muted-foreground'
            }`}
            onClick={() => {
              setActiveTab('existing');
              setSearchQuery('');
              setSelectedBranch(null);
            }}>
            {t('features.modal.tab.existing')}
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto min-h-0">
          {activeTab === 'new' ? (
            <div className="flex flex-col gap-3">
              <div>
                <label className="block text-[13px] font-medium mb-1 text-foreground">
                  {t('features.modal.label.branch_name')}
                </label>
                <input
                  type="text"
                  placeholder={t('features.modal.placeholder.branch_name')}
                  value={branchName}
                  onChange={(e) => setBranchName(e.target.value)}
                  className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-[13px] font-medium mb-1 text-foreground">
                  {t('features.modal.label.base_branch')}
                </label>
                <input
                  type="text"
                  placeholder={t('common.filter')}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none mb-2"
                />
                {isLoadingBranches ? (
                  <div className="p-3 text-center text-muted-foreground">
                    {t('features.modal.loading_branches')}
                  </div>
                ) : (
                  <div className="max-h-[200px] overflow-y-auto">
                    {filteredBaseBranches.map((b) => (
                      <button
                        key={b.name}
                        onClick={() => setBaseBranch(b.name)}
                        className={`flex items-center gap-2 w-full px-3 py-2 rounded-lg cursor-pointer text-[13px] text-foreground text-left mb-1.5 ${
                          baseBranch === b.name
                            ? 'border border-primary bg-transparent'
                            : 'border border-border bg-transparent'
                        }`}>
                        <span className="flex-1">{b.name}</span>
                        <span
                          className={`text-[10px] px-1.5 py-0.5 rounded-[10px] ${
                            b.isRemote
                              ? 'bg-blue-500/15 text-blue-500'
                              : 'bg-green-500/15 text-green-500'
                          }`}>
                          {b.isRemote ? b.remoteName || t('features.branch_badge.remote') : t('features.branch_badge.local')}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              <input
                type="text"
                placeholder={t('features.modal.search_placeholder')}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full px-3 py-2 border border-border rounded-lg bg-input text-foreground text-sm outline-none"
                autoFocus
              />
              {isLoadingBranches ? (
                <div className="p-3 text-center text-muted-foreground">
                  {t('features.modal.loading_branches')}
                </div>
              ) : filteredBranches.length === 0 ? (
                <div className="p-6 text-center text-muted-foreground text-[13px]">
                  {searchQuery
                    ? t('features.modal.no_matching_branches')
                    : t('features.modal.all_branches_have_worktrees')}
                </div>
              ) : (
                <div className="max-h-[300px] overflow-y-auto">
                  {filteredBranches.map((b) => (
                    <button
                      key={`${b.name}-${b.isRemote}`}
                      onClick={() => setSelectedBranch(b.name)}
                      className={`flex items-center gap-2 w-full px-3 py-2 rounded-lg cursor-pointer text-[13px] text-foreground text-left mb-1.5 ${
                        selectedBranch === b.name
                          ? 'border border-primary bg-transparent'
                          : 'border border-border bg-transparent'
                      }`}>
                      <span className="flex-1">{b.name}</span>
                      <span
                        className={`text-[10px] px-1.5 py-0.5 rounded-[10px] ${
                          b.isRemote
                            ? 'bg-blue-500/15 text-blue-500'
                            : 'bg-green-500/15 text-green-500'
                        }`}>
                        {b.isRemote ? b.remoteName || t('features.branch_badge.remote') : t('features.branch_badge.local')}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {error && (
          <p className="text-destructive text-[13px] mt-3 mb-0">{error}</p>
        )}

        {/* Actions */}
        <div className="flex gap-3 justify-end mt-4">
          <button
            onClick={onClose}
            className="px-4 py-2 border border-border rounded-lg bg-transparent text-foreground text-sm cursor-pointer">
            {t('common.cancel')}
          </button>
          <button
            onClick={activeTab === 'new' ? createNewBranch : attachExistingBranch}
            disabled={isCreating || (activeTab === 'existing' && !selectedBranch)}
            className={`px-4 py-2 border-none rounded-lg bg-primary text-primary-foreground text-sm font-medium cursor-pointer ${
              isCreating || (activeTab === 'existing' && !selectedBranch) ? 'opacity-50 cursor-not-allowed' : ''
            }`}>
            {isCreating
              ? t('common.creating')
              : activeTab === 'new'
                ? t('common.create')
                : t('features.modal.attach')}
          </button>
        </div>
      </div>
    </div>
  );
}
