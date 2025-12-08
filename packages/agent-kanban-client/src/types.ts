export interface AgentEvent {
    eventType: string;
    sourceAgent: string;
    sessionId: string;
    projectDir: string;
    toolName?: string;
    payload?: any;
    featureId?: string;
}

export interface SessionStart {
    sessionId: string;
    sourceAgent: string;
    projectDir: string;
}

export interface FeatureStats {
    total: number;
    completed: number;
    percentage: number;
}

export interface ChangedFeature {
    description: string;
    category: string;
}

export interface Feature {
    id: string;
    projectDir: string;
    description: string;
    category: string;
    passes: boolean;
    inProgress: boolean;
    agent?: string;
    steps?: string[];
    workCount: number;
    completionCriteria?: string;
    updatedAt: string;
}

export interface FeatureUpdateEvent {
    projectDir: string;
    stats: FeatureStats;
    changedFeatures: ChangedFeature[];
}

export interface SessionEnd {
    sessionId: string;
    sourceAgent?: string;
    projectDir?: string;
}
