## Tauri Shell + Phoenix Backend: The Hybrid Architecture

This pattern gives you:
- **Native desktop app** feel (system tray, file access, notifications)
- **LiveView real-time UI** (no state sync bugs)
- **Single distributable** (Tauri bundles Phoenix binary)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         USER'S MACHINE                               │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                      TAURI APP                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │                    RUST CORE (thin)                       │  │ │
│  │  │  • Spawn Phoenix as child process                         │  │ │
│  │  │  • System tray management                                 │  │ │
│  │  │  • Native file dialogs                                    │  │ │
│  │  │  • OS notifications                                       │  │ │
│  │  │  • Deep links (ijoka://open-project)                      │  │ │
│  │  │  • Auto-update                                            │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                           │                                     │ │
│  │                           │ spawn + manage                      │ │
│  │                           ▼                                     │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │                 PHOENIX SERVER                            │  │ │
│  │  │                 (Burrito binary)                          │  │ │
│  │  │                 localhost:4000                            │  │ │
│  │  │                                                           │  │ │
│  │  │  • LiveView UI (Kanban, Graph, Timeline, Table)          │  │ │
│  │  │  • Ash Resources (Feature, Session, Event, Rule)         │  │ │
│  │  │  • GenServer per agent session                           │  │ │
│  │  │  • MCP server (for Claude Code, etc.)                    │  │ │
│  │  │  • PubSub for real-time updates                          │  │ │
│  │  │  • SQLite via Ecto (or Postgres for power users)         │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                           │                                     │ │
│  │                           │ HTTP/WebSocket                      │ │
│  │                           ▼                                     │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │                    WEBVIEW                                │  │ │
│  │  │              points to localhost:4000                     │  │ │
│  │  │                                                           │  │ │
│  │  │  • LiveView handles all UI                                │  │ │
│  │  │  • Phoenix.JS for WebSocket                               │  │ │
│  │  │  • Minimal JS (hooks for native integration)              │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                    CLAUDE CODE                                  │ │
│  │                    (separate process)                           │ │
│  │                                                                 │ │
│  │  Connects to Phoenix MCP server via stdio or HTTP              │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## How Tauri Spawns Phoenix

### Rust Side (Tauri)

```rust
// src-tauri/src/phoenix.rs

use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use tauri::api::path::app_data_dir;

pub struct PhoenixServer {
    process: Child,
    port: u16,
}

impl PhoenixServer {
    pub fn start(app_handle: &tauri::AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let port = find_available_port(4000..4100)?;
        
        // Phoenix binary bundled in resources
        let phoenix_binary = app_handle
            .path_resolver()
            .resolve_resource("bin/ijoka_server")
            .expect("Phoenix binary not found");
        
        // Data directory for SQLite, etc.
        let data_dir = app_data_dir(&app_handle.config())
            .expect("Failed to get app data dir")
            .join("ijoka");
        
        std::fs::create_dir_all(&data_dir)?;
        
        let process = Command::new(&phoenix_binary)
            .env("PORT", port.to_string())
            .env("DATABASE_PATH", data_dir.join("ijoka.db").to_str().unwrap())
            .env("SECRET_KEY_BASE", generate_secret_key())
            .env("PHX_HOST", "localhost")
            .env("MCP_SOCKET_PATH", data_dir.join("mcp.sock").to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        // Wait for server to be ready
        wait_for_server(port, std::time::Duration::from_secs(10))?;
        
        Ok(Self { process, port })
    }
    
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
    
    pub fn shutdown(&mut self) -> Result<(), std::io::Error> {
        // Graceful shutdown via HTTP endpoint
        let _ = reqwest::blocking::post(format!("{}/api/shutdown", self.url()));
        
        // Then kill if needed
        self.process.kill()
    }
}

fn find_available_port(range: std::ops::Range<u16>) -> Result<u16, &'static str> {
    for port in range {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err("No available port found")
}

fn wait_for_server(port: u16, timeout: std::time::Duration) -> Result<(), &'static str> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if reqwest::blocking::get(format!("http://localhost:{}/health", port)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Server failed to start")
}
```

### Main Tauri Setup

```rust
// src-tauri/src/main.rs

mod phoenix;

use phoenix::PhoenixServer;
use tauri::{Manager, SystemTray, SystemTrayEvent, CustomMenuItem, SystemTrayMenu};

fn main() {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("open", "Open Ijoka"))
        .add_item(CustomMenuItem::new("quit", "Quit"));
    
    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                match id.as_str() {
                    "open" => {
                        if let Some(window) = app.get_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                    "quit" => {
                        // Shutdown Phoenix before exit
                        let state = app.state::<PhoenixState>();
                        state.server.lock().unwrap().shutdown().ok();
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        })
        .setup(|app| {
            // Start Phoenix server
            let server = PhoenixServer::start(&app.handle())?;
            let url = server.url();
            
            // Store in state for later access
            app.manage(PhoenixState {
                server: std::sync::Mutex::new(server),
            });
            
            // Point WebView to Phoenix
            let window = app.get_window("main").unwrap();
            window.eval(&format!("window.location.href = '{}'", url))?;
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_file_dialog,
            show_notification,
            get_phoenix_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

struct PhoenixState {
    server: std::sync::Mutex<PhoenixServer>,
}

#[tauri::command]
fn get_phoenix_url(state: tauri::State<PhoenixState>) -> String {
    state.server.lock().unwrap().url()
}

#[tauri::command]
async fn open_file_dialog() -> Option<String> {
    tauri::api::dialog::blocking::FileDialogBuilder::new()
        .pick_folder()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn show_notification(title: String, body: String) {
    tauri::api::notification::Notification::new("com.ijoka.app")
        .title(&title)
        .body(&body)
        .show()
        .ok();
}
```

---

## Phoenix Side

### Burrito Build Configuration

```elixir
# mix.exs

defmodule Ijoka.MixProject do
  use Mix.Project

  def project do
    [
      app: :ijoka,
      version: "1.0.0",
      elixir: "~> 1.16",
      releases: releases(),
      # ...
    ]
  end

  defp releases do
    [
      ijoka_server: [
        steps: [:assemble, &Burrito.wrap/1],
        burrito: [
          targets: [
            macos_arm64: [os: :darwin, cpu: :aarch64],
            macos_x86_64: [os: :darwin, cpu: :x86_64],
            linux_x86_64: [os: :linux, cpu: :x86_64],
            windows_x86_64: [os: :windows, cpu: :x86_64],
          ]
        ]
      ]
    ]
  end
end
```

Build produces:
```
_build/prod/
├── ijoka_server_macos_arm64      # ~50MB self-contained binary
├── ijoka_server_macos_x86_64
├── ijoka_server_linux_x86_64
└── ijoka_server_windows_x86_64.exe
```

### Phoenix Endpoint Configuration

```elixir
# lib/ijoka_web/endpoint.ex

defmodule IjokaWeb.Endpoint do
  use Phoenix.Endpoint, otp_app: :ijoka

  # LiveView socket
  socket "/live", Phoenix.LiveView.Socket,
    websocket: [connect_info: [:peer_data, :uri, session: @session_options]]

  # MCP socket for Claude Code
  socket "/mcp", IjokaWeb.MCPSocket,
    websocket: true

  plug Plug.Static,
    at: "/",
    from: :ijoka,
    gzip: true

  plug Plug.Session, @session_options
  plug IjokaWeb.Router
end
```

### Health Check for Tauri

```elixir
# lib/ijoka_web/router.ex

scope "/api", IjokaWeb do
  pipe_through :api
  
  get "/health", HealthController, :check
  post "/shutdown", HealthController, :shutdown
end

# lib/ijoka_web/controllers/health_controller.ex

defmodule IjokaWeb.HealthController do
  use IjokaWeb, :controller

  def check(conn, _params) do
    json(conn, %{status: "ok", version: Application.spec(:ijoka, :vsn)})
  end

  def shutdown(conn, _params) do
    # Graceful shutdown
    spawn(fn ->
      Process.sleep(100)
      System.stop(0)
    end)
    json(conn, %{status: "shutting_down"})
  end
end
```

---

## LiveView UI (Replaces Vue)

### Main Layout with View Switcher

```elixir
# lib/ijoka_web/live/dashboard_live.ex

defmodule IjokaWeb.DashboardLive do
  use IjokaWeb, :live_view

  alias Ijoka.Projects
  alias Ijoka.Features

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      # Subscribe to real-time updates
      Phoenix.PubSub.subscribe(Ijoka.PubSub, "features")
      Phoenix.PubSub.subscribe(Ijoka.PubSub, "events")
      Phoenix.PubSub.subscribe(Ijoka.PubSub, "sessions")
    end

    project = Projects.get_current()
    features = Features.list_by_project(project.id)

    {:ok,
     socket
     |> assign(:project, project)
     |> assign(:current_view, :kanban)
     |> stream(:features, features)
     |> stream(:events, [])}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="dashboard">
      <header class="dashboard__header">
        <h1>Ijoka</h1>
        <.view_switcher current={@current_view} />
        <.progress_bar features={@streams.features} />
      </header>

      <main class="dashboard__main">
        <%= case @current_view do %>
          <% :kanban -> %>
            <.live_component module={IjokaWeb.KanbanLive} id="kanban" features={@streams.features} />
          <% :graph -> %>
            <.live_component module={IjokaWeb.GraphLive} id="graph" features={@streams.features} />
          <% :timeline -> %>
            <.live_component module={IjokaWeb.TimelineLive} id="timeline" events={@streams.events} />
          <% :table -> %>
            <.live_component module={IjokaWeb.TableLive} id="table" features={@streams.features} />
          <% :rules -> %>
            <.live_component module={IjokaWeb.RulesLive} id="rules" project={@project} />
        <% end %>
      </main>
    </div>
    """
  end

  @impl true
  def handle_event("switch_view", %{"view" => view}, socket) do
    {:noreply, assign(socket, :current_view, String.to_existing_atom(view))}
  end

  # Real-time updates from PubSub
  @impl true
  def handle_info({:feature_updated, feature}, socket) do
    {:noreply, stream_insert(socket, :features, feature)}
  end

  def handle_info({:feature_created, feature}, socket) do
    {:noreply, stream_insert(socket, :features, feature, at: 0)}
  end

  def handle_info({:event_created, event}, socket) do
    {:noreply, stream_insert(socket, :events, event, at: 0)}
  end
end
```

### Kanban Component

```elixir
# lib/ijoka_web/live/kanban_live.ex

defmodule IjokaWeb.KanbanLive do
  use IjokaWeb, :live_component

  alias Ijoka.Features

  @statuses [:pending, :in_progress, :blocked, :complete]

  @impl true
  def render(assigns) do
    ~H"""
    <div class="kanban" phx-hook="Sortable" id="kanban-board">
      <%= for status <- @statuses do %>
        <div class="kanban__column" data-status={status}>
          <h3 class="kanban__column-title">
            <%= humanize(status) %>
            <span class="kanban__count"><%= count_by_status(@features, status) %></span>
          </h3>
          
          <div class="kanban__cards" id={"column-#{status}"} phx-hook="Sortable" data-status={status}>
            <div
              :for={{dom_id, feature} <- @features}
              :if={feature.status == status}
              class="kanban__card"
              id={dom_id}
              draggable="true"
              phx-click="select_feature"
              phx-value-id={feature.id}
            >
              <.feature_card feature={feature} />
            </div>
          </div>
        </div>
      <% end %>
    </div>
    """
  end

  @impl true
  def handle_event("reorder", %{"id" => id, "status" => new_status}, socket) do
    feature = Features.get!(id)
    {:ok, updated} = Features.update_status(feature, String.to_existing_atom(new_status))
    
    # PubSub broadcast happens in Ash notifier - UI updates automatically everywhere
    {:noreply, socket}
  end

  defp count_by_status(features, status) do
    features
    |> Enum.count(fn {_, f} -> f.status == status end)
  end
end
```

### Graph View with D3 Hook

```elixir
# lib/ijoka_web/live/graph_live.ex

defmodule IjokaWeb.GraphLive do
  use IjokaWeb, :live_component

  alias Ijoka.Features

  @impl true
  def update(assigns, socket) do
    # Prepare graph data for D3
    features = stream_to_list(assigns.features)
    
    graph_data = %{
      nodes: Enum.map(features, &feature_to_node/1),
      edges: Features.get_dependencies(features) |> Enum.map(&dep_to_edge/1)
    }

    {:ok,
     socket
     |> assign(assigns)
     |> push_event("graph_data", graph_data)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div 
      id="graph-container" 
      phx-hook="D3Graph"
      phx-update="ignore"
      class="graph-view"
    >
      <!-- D3 renders here -->
    </div>
    """
  end

  defp feature_to_node(feature) do
    %{
      id: feature.id,
      label: String.slice(feature.description, 0, 30),
      status: feature.status,
      category: feature.category
    }
  end

  defp dep_to_edge({from_id, to_id}) do
    %{source: from_id, target: to_id, type: "depends_on"}
  end
end
```

```javascript
// assets/js/hooks/d3_graph.js

import * as d3 from "d3";

export const D3Graph = {
  mounted() {
    this.svg = d3.select(this.el).append("svg");
    this.simulation = null;
    
    this.handleEvent("graph_data", ({ nodes, edges }) => {
      this.renderGraph(nodes, edges);
    });
  },

  renderGraph(nodes, edges) {
    // D3 force-directed graph rendering
    // ... standard D3 code
  },

  destroyed() {
    if (this.simulation) {
      this.simulation.stop();
    }
  }
};
```

---

## Native Integration via Tauri Commands

LiveView can call Tauri commands for native features:

```javascript
// assets/js/hooks/native.js

export const NativeIntegration = {
  mounted() {
    // Listen for LiveView events that need native handling
    this.handleEvent("open_folder_dialog", async () => {
      if (window.__TAURI__) {
        const path = await window.__TAURI__.invoke("open_file_dialog");
        if (path) {
          this.pushEvent("folder_selected", { path });
        }
      }
    });

    this.handleEvent("show_notification", ({ title, body }) => {
      if (window.__TAURI__) {
        window.__TAURI__.invoke("show_notification", { title, body });
      } else {
        // Fallback to browser notification
        new Notification(title, { body });
      }
    });
  }
};
```

```elixir
# In LiveView
def handle_event("add_project", _params, socket) do
  {:noreply, push_event(socket, "open_folder_dialog", %{})}
end

def handle_event("folder_selected", %{"path" => path}, socket) do
  {:ok, project} = Projects.create(%{path: path, name: Path.basename(path)})
  {:noreply, stream_insert(socket, :projects, project)}
end

def handle_event("feature_completed", %{"id" => id}, socket) do
  feature = Features.get!(id)
  {:ok, _} = Features.complete(feature)
  
  # Native notification
  {:noreply, push_event(socket, "show_notification", %{
    title: "Feature Complete! 🎉",
    body: feature.description
  })}
end
```

---

## MCP Server as GenServer

```elixir
# lib/ijoka/mcp/server.ex

defmodule Ijoka.MCP.Server do
  use GenServer
  
  alias Ijoka.Features
  alias Ijoka.Sessions
  alias Ijoka.Events

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  def init(_opts) do
    # Listen on Unix socket for Claude Code
    socket_path = Application.get_env(:ijoka, :mcp_socket_path)
    {:ok, listen_socket} = :gen_tcp.listen(0, [
      :binary,
      packet: :line,
      active: false,
      reuseaddr: true,
      ip: {:local, socket_path}
    ])
    
    # Accept connections in separate process
    spawn_link(fn -> accept_loop(listen_socket) end)
    
    {:ok, %{sessions: %{}}}
  end

  defp accept_loop(listen_socket) do
    {:ok, client} = :gen_tcp.accept(listen_socket)
    spawn(fn -> handle_client(client) end)
    accept_loop(listen_socket)
  end

  defp handle_client(socket) do
    case :gen_tcp.recv(socket, 0) do
      {:ok, data} ->
        request = Jason.decode!(data)
        response = handle_mcp_request(request)
        :gen_tcp.send(socket, Jason.encode!(response) <> "\n")
        handle_client(socket)
      {:error, :closed} ->
        :ok
    end
  end

  defp handle_mcp_request(%{"method" => "tools/list"}) do
    %{
      tools: [
        %{name: "ijoka_status", description: "Get project status", inputSchema: %{...}},
        %{name: "ijoka_start_feature", description: "Start a feature", inputSchema: %{...}},
        %{name: "ijoka_complete_feature", description: "Complete a feature", inputSchema: %{...}},
        %{name: "ijoka_block_feature", description: "Block a feature", inputSchema: %{...}},
        %{name: "ijoka_record_insight", description: "Record insight", inputSchema: %{...}},
      ]
    }
  end

  defp handle_mcp_request(%{"method" => "tools/call", "params" => %{"name" => "ijoka_status"}}) do
    project = Projects.get_current()
    features = Features.list_by_project(project.id)
    
    %{
      content: [%{
        type: "text",
        text: Jason.encode!(%{
          project: project.path,
          current_feature: Features.get_active(project.id),
          progress: %{
            total: length(features),
            complete: Enum.count(features, & &1.status == :complete),
            in_progress: Enum.count(features, & &1.status == :in_progress),
            blocked: Enum.count(features, & &1.status == :blocked),
            pending: Enum.count(features, & &1.status == :pending)
          }
        })
      }]
    }
  end

  defp handle_mcp_request(%{"method" => "tools/call", "params" => %{"name" => "ijoka_complete_feature", "arguments" => args}}) do
    feature = case args["feature_id"] do
      nil -> Features.get_active(Projects.get_current().id)
      id -> Features.get!(id)
    end
    
    {:ok, updated} = Features.complete(feature)
    
    # This broadcasts via PubSub - LiveView updates automatically
    %{content: [%{type: "text", text: Jason.encode!(%{completed: true, feature_id: updated.id})}]}
  end
end
```

---

## Ash Resources (Replace Graph DB Schema)

```elixir
# lib/ijoka/resources/feature.ex

defmodule Ijoka.Feature do
  use Ash.Resource,
    data_layer: AshSqlite.DataLayer,
    notifiers: [Ijoka.Notifiers.PubSubNotifier]

  sqlite do
    table "features"
    repo Ijoka.Repo
  end

  attributes do
    uuid_primary_key :id
    attribute :description, :string, allow_nil?: false
    attribute :category, :atom, constraints: [
      one_of: [:functional, :ui, :security, :performance, :documentation, :testing, :infrastructure, :refactoring]
    ]
    attribute :status, :atom, constraints: [
      one_of: [:pending, :in_progress, :blocked, :complete]
    ], default: :pending
    attribute :priority, :integer, default: 0
    attribute :steps, {:array, :string}, default: []
    attribute :block_reason, :string
    
    create_timestamp :created_at
    update_timestamp :updated_at
  end

  relationships do
    belongs_to :project, Ijoka.Project
    has_many :events, Ijoka.Event
    
    # Graph-like relationship
    many_to_many :depends_on, Ijoka.Feature do
      through Ijoka.FeatureDependency
      source_attribute_on_join_resource :feature_id
      destination_attribute_on_join_resource :depends_on_id
    end
    
    many_to_many :dependents, Ijoka.Feature do
      through Ijoka.FeatureDependency
      source_attribute_on_join_resource :depends_on_id
      destination_attribute_on_join_resource :feature_id
    end
  end

  actions do
    defaults [:read, :destroy]
    
    create :create do
      accept [:description, :category, :priority, :steps, :project_id]
      
      change fn changeset, _ ->
        # Append FeatureCreated event
        Ash.Changeset.after_action(changeset, fn _changeset, feature ->
          Ijoka.Event.create!(%{
            type: :feature_created,
            feature_id: feature.id,
            payload: %{description: feature.description}
          })
          {:ok, feature}
        end)
      end
    end
    
    update :start do
      accept []
      
      validate attribute_equals(:status, :pending) do
        message "Can only start pending features"
      end
      
      change set_attribute(:status, :in_progress)
      change Ijoka.Changes.AppendEvent, type: :feature_started
    end
    
    update :complete do
      accept []
      
      validate attribute_equals(:status, :in_progress) do
        message "Can only complete in-progress features"
      end
      
      change set_attribute(:status, :complete)
      change Ijoka.Changes.AppendEvent, type: :feature_completed
    end
    
    update :block do
      accept [:block_reason]
      
      change set_attribute(:status, :blocked)
      change Ijoka.Changes.AppendEvent, type: :feature_blocked
    end
    
    update :unblock do
      accept []
      
      change set_attribute(:status, :pending)
      change set_attribute(:block_reason, nil)
      change Ijoka.Changes.AppendEvent, type: :feature_unblocked
    end
  end

  calculations do
    calculate :is_blocked_by_dependencies, :boolean, fn records, _ ->
      Enum.map(records, fn record ->
        deps = Ash.load!(record, :depends_on).depends_on
        Enum.any?(deps, & &1.status != :complete)
      end)
    end
  end
end
```

### PubSub Notifier (Automatic Real-time Updates)

```elixir
# lib/ijoka/notifiers/pubsub_notifier.ex

defmodule Ijoka.Notifiers.PubSubNotifier do
  use Ash.Notifier

  def notify(%Ash.Notifier.Notification{resource: Ijoka.Feature, action: action, data: feature}) do
    event = case action.name do
      :create -> :feature_created
      :start -> :feature_updated
      :complete -> :feature_updated
      :block -> :feature_updated
      :unblock -> :feature_updated
      _ -> :feature_updated
    end
    
    Phoenix.PubSub.broadcast(Ijoka.PubSub, "features", {event, feature})
    :ok
  end
end
```

---

## Build & Distribution

### 1. Build Phoenix Binary

```bash
# Build Burrito binary for current platform
cd ijoka-phoenix
MIX_ENV=prod mix release ijoka_server
```

### 2. Bundle in Tauri

```json
// tauri.conf.json
{
  "bundle": {
    "resources": [
      "bin/ijoka_server*"
    ]
  }
}
```

### 3. Build Script

```bash
#!/bin/bash
# build.sh

# Build Phoenix for all platforms
cd ijoka-phoenix
for target in macos_arm64 macos_x86_64 linux_x86_64 windows_x86_64; do
  MIX_ENV=prod mix release ijoka_server --target $target
  cp _build/prod/ijoka_server_$target ../ijoka-tauri/bin/
done

# Build Tauri
cd ../ijoka-tauri
pnpm tauri build
```

Final artifact: Single installer (~80-100MB) containing:
- Tauri shell
- Phoenix binary
- SQLite bundled
- All assets

---

## What You Gain vs Current Architecture

| Current Pain | Phoenix Solution |
|--------------|------------------|
| Vue ↔ SQLite state sync bugs | LiveView (single source) |
| WebSocket boilerplate | Built into Phoenix |
| File watcher race conditions | PubSub notifications |
| MCP server reliability | OTP supervision |
| Multi-agent coordination | GenServers with fault tolerance |
| Graph queries need separate DB | Ash relationships + SQLite |
| Event sourcing complexity | Ash notifiers + simple append |
| Cache invalidation | No cache—just broadcast |

## What You Keep

| From Tauri | Purpose |
|------------|---------|
| Native window chrome | Desktop app feel |
| System tray | Background presence |
| File dialogs | Native OS integration |
| Notifications | OS-level alerts |
| Deep links | `ijoka://` protocol |
| Auto-update | Seamless upgrades |
| Code signing | macOS/Windows trust |

---

## Migration Path

**Phase 1**: Parallel Development
- Keep current Tauri/Vue app working
- Build Phoenix version alongside
- Share SQLite database

**Phase 2**: Feature Parity
- Port all views to LiveView
- Implement MCP server in Phoenix
- Test with Claude Code

**Phase 3**: Switch WebView
- Point Tauri WebView to Phoenix
- Remove Vue build step
- Test native integration hooks

**Phase 4**: Remove Old Code
- Delete Vue components
- Delete Rust HTTP server
- Keep only Rust native integration

