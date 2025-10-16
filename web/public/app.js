class QWebUI {
    constructor() {
        this.workerId = null;
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        
        this.elements = {
            connectionStatus: document.getElementById('connection-status'),
            connectionText: document.getElementById('connection-text'),
            workerName: document.getElementById('worker-name'),
            workerState: document.getElementById('worker-state'),
            output: document.getElementById('output'),
            promptInput: document.getElementById('prompt-input'),
            sendButton: document.getElementById('send-button'),
            cancelButton: document.getElementById('cancel-button'),
        };
        
        this.init();
    }
    
    async init() {
        // Fetch worker list
        const workers = await this.fetchWorkers();
        if (workers.length === 0) {
            this.showError('No workers found');
            return;
        }
        
        // Connect to first worker
        this.workerId = workers[0].worker_id;
        this.elements.workerName.textContent = workers[0].name;
        
        // Setup event listeners
        this.setupEventListeners();
        
        // Connect WebSocket
        this.connect();
    }
    
    async fetchWorkers() {
        try {
            const response = await fetch('/api/workers');
            const data = await response.json();
            return data.workers;
        } catch (error) {
            console.error('Failed to fetch workers:', error);
            return [];
        }
    }
    
    setupEventListeners() {
        this.elements.sendButton.addEventListener('click', () => this.sendPrompt());
        this.elements.cancelButton.addEventListener('click', () => this.cancelJob());
        
        this.elements.promptInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.sendPrompt();
            }
        });
    }
    
    connect() {
        this.updateConnectionStatus('connecting');
        
        this.ws = new WebSocket(`ws://${window.location.host}/ws/worker/${this.workerId}`);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.updateConnectionStatus('connected');
            this.reconnectAttempts = 0;
            this.reconnectDelay = 1000;
        };
        
        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.handleEvent(data);
        };
        
        this.ws.onclose = (event) => {
            console.log('WebSocket disconnected');
            this.updateConnectionStatus('disconnected');
            
            // Don't reconnect on normal closure (code 1000)
            if (event.code !== 1000) {
                this.reconnect();
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }
    
    reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            this.showError('Failed to reconnect after multiple attempts');
            return;
        }
        
        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
        
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        
        setTimeout(() => {
            this.connect();
        }, delay);
    }
    
    handleEvent(event) {
        console.log('Event:', event);
        
        switch (event.type) {
            case 'worker_state_snapshot':
                this.handleSnapshot(event);
                break;
            case 'worker_state_changed':
                this.updateWorkerState(event.new_state);
                break;
            case 'job_started':
                this.handleJobStarted(event);
                break;
            case 'job_completed':
                this.handleJobCompleted(event);
                break;
            case 'output_chunk':
                this.handleOutputChunk(event);
                break;
        }
    }
    
    handleSnapshot(snapshot) {
        this.elements.workerName.textContent = snapshot.name;
        this.updateWorkerState(snapshot.lifecycle_state);
        
        // Clear output for fresh start
        this.elements.output.innerHTML = '';
    }
    
    updateWorkerState(state) {
        this.elements.workerState.textContent = state;
        this.elements.workerState.className = `state-badge ${state}`;
        
        // Update input controls
        const isIdle = state === 'idle' || state === 'idle_failed';
        const isBusy = state === 'busy';
        
        this.elements.promptInput.disabled = !isIdle;
        this.elements.sendButton.disabled = !isIdle;
        this.elements.cancelButton.disabled = !isBusy;
    }
    
    handleJobStarted(event) {
        // Clear previous output
        this.elements.output.innerHTML = '';
    }
    
    handleJobCompleted(event) {
        const result = event.result;
        if (result.status === 'failed') {
            this.appendOutput('Error: ' + result.error, 'error');
        }
    }
    
    handleOutputChunk(event) {
        const chunk = event.chunk;
        
        switch (chunk.chunk_type) {
            case 'assistant_response':
                this.appendOutput(chunk.text, 'assistant');
                break;
            case 'tool_use':
                this.appendOutput(
                    `Tool: ${chunk.tool_name}\nInput: ${JSON.stringify(chunk.tool_input, null, 2)}`,
                    'tool-use'
                );
                break;
            case 'tool_result':
                this.appendOutput(
                    `Tool Result: ${chunk.tool_name}\n${chunk.result}`,
                    'tool-result'
                );
                break;
        }
    }
    
    appendOutput(text, type = 'assistant') {
        const chunk = document.createElement('div');
        chunk.className = `output-chunk ${type}`;
        chunk.textContent = text;
        this.elements.output.appendChild(chunk);
        
        // Scroll to bottom
        this.elements.output.parentElement.scrollTop = this.elements.output.parentElement.scrollHeight;
    }
    
    sendPrompt() {
        const text = this.elements.promptInput.value.trim();
        if (!text) return;
        
        const command = {
            type: 'prompt',
            text: text
        };
        
        this.ws.send(JSON.stringify(command));
        this.elements.promptInput.value = '';
    }
    
    cancelJob() {
        const command = {
            type: 'cancel'
        };
        
        this.ws.send(JSON.stringify(command));
    }
    
    updateConnectionStatus(status) {
        this.elements.connectionStatus.className = `status-indicator ${status}`;
        
        const statusText = {
            connecting: 'Connecting...',
            connected: 'Connected',
            disconnected: 'Disconnected'
        };
        
        this.elements.connectionText.textContent = statusText[status] || status;
    }
    
    showError(message) {
        this.elements.output.innerHTML = `<div class="output-chunk error">${message}</div>`;
    }
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    new QWebUI();
});
