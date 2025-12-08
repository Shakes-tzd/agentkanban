import axios, { AxiosInstance } from 'axios';
import { AgentEvent, SessionStart, FeatureUpdateEvent, SessionEnd, Feature } from './types';

export class AgentKanbanClient {
    private client: AxiosInstance;
    private baseUrl: string;

    constructor(baseUrl: string = 'http://127.0.0.1:4000') {
        this.baseUrl = baseUrl;
        this.client = axios.create({
            baseURL: baseUrl,
            headers: {
                'Content-Type': 'application/json',
            },
            timeout: 2000, // Short timeout to avoid hanging if server is down
        });
    }

    /**
     * Check if the AgentKanban server is running
     */
    async health(): Promise<boolean> {
        try {
            const response = await this.client.get('/health');
            return response.status === 200;
        } catch (e) {
            return false;
        }
    }

    /**
     * Start a new session
     */
    async startSession(event: SessionStart): Promise<boolean> {
        const success = await this.postSafe('/sessions/start', event);
        if (success) {
            console.log(`[AgentKanban] Session started: ${event.sessionId}`);
        }
        return success;
    }

    /**
     * Get features for a project
     */
    async getFeatures(projectDir?: string): Promise<Feature[]> {
        try {
            const params = projectDir ? { project_dir: projectDir } : {};
            const response = await this.client.get('/features', { params });
            return response.data;
        } catch (e) {
            console.error('[AgentKanban] Failed to fetch features');
            return [];
        }
    }

    /**
     * Send an event (tool use, etc)
     */
    async sendEvent(event: AgentEvent): Promise<boolean> {
        return this.postSafe('/events', event);
    }

    /**
     * End a session
     */
    async endSession(event: SessionEnd): Promise<boolean> {
        const success = await this.postSafe('/sessions/end', event);
        if (success) {
            console.log(`[AgentKanban] Session ended: ${event.sessionId}`);
        }
        return success;
    }

    /**
     * Update feature progress
     */
    async updateFeature(event: FeatureUpdateEvent): Promise<boolean> {
        return this.postSafe('/events/feature-update', event);
    }

    private async postSafe(path: string, data: any): Promise<boolean> {
        try {
            await this.client.post(path, data);
            return true;
        } catch (error) {
            // Fail silently but log to debug if needed, we don't want to crash the agent
            // if the dashboard is not running
            // console.debug(`[AgentKanban] Failed to send to ${path}:`, error);
            return false;
        }
    }
}
