defmodule IjokaWeb.Graph.EventSubscriber do
  @moduledoc """
  GenServer that polls Memgraph for new events and broadcasts them via PubSub.
  LiveView components subscribe to receive real-time updates.
  Uses event ID tracking (not timestamps) to prevent duplicate broadcasts.
  """
  use GenServer

  require Logger

  alias IjokaWeb.Graph.Memgraph

  @poll_interval 2_000  # Poll every 2 seconds
  @max_seen_ids 500     # Keep last N event IDs to prevent memory growth

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @doc """
  Subscribe to event updates for a specific project.
  """
  def subscribe(project_path) do
    Phoenix.PubSub.subscribe(IjokaWeb.PubSub, "events:#{project_path}")
  end

  @doc """
  Subscribe to all events (project-agnostic).
  """
  def subscribe_all do
    Phoenix.PubSub.subscribe(IjokaWeb.PubSub, "events:all")
  end

  # GenServer callbacks

  @impl true
  def init(_state) do
    # Start with empty set of seen event IDs
    schedule_poll()
    {:ok, %{seen_event_ids: MapSet.new()}}
  end

  @impl true
  def handle_info(:poll, state) do
    new_state = poll_events(state)
    schedule_poll()
    {:noreply, new_state}
  end

  defp schedule_poll do
    Process.send_after(self(), :poll, @poll_interval)
  end

  defp poll_events(state) do
    case Memgraph.get_events(20) do
      {:ok, events} when events != [] ->
        # Filter to only events we haven't seen before (by ID, not timestamp)
        {new_events, updated_seen} = filter_unseen_events(events, state.seen_event_ids)

        # Debug: show first event structure
        if length(events) > 0 do
          first_event = hd(events)
          first_id = Map.get(first_event, :id) || Map.get(first_event, "id")
          Logger.debug("First event ID: #{inspect(first_id)}, keys: #{inspect(Map.keys(first_event))}")
        end
        Logger.debug("Poll: got #{length(events)} events, #{length(new_events)} new, seen_ids size: #{MapSet.size(state.seen_event_ids)}")

        if length(new_events) > 0 do
          for event <- new_events do
            # Broadcast to project-specific channel
            if event[:project_dir] do
              Phoenix.PubSub.broadcast(
                IjokaWeb.PubSub,
                "events:#{event[:project_dir]}",
                {:new_event, event}
              )
            end

            # Also broadcast to global channel
            Phoenix.PubSub.broadcast(
              IjokaWeb.PubSub,
              "events:all",
              {:new_event, event}
            )
          end

          unique_ids = new_events |> Enum.map(fn e -> Map.get(e, :id) || Map.get(e, "id") end) |> Enum.uniq()
          Logger.debug("Broadcast #{length(new_events)} new events with #{length(unique_ids)} unique IDs: #{inspect(Enum.take(unique_ids, 5))}...")
        end

        %{state | seen_event_ids: updated_seen}

      {:ok, []} ->
        state

      {:error, reason} ->
        Logger.warning("Failed to poll events: #{inspect(reason)}")
        state
    end
  end

  # Filter events by ID (not timestamp) - much more reliable for deduplication
  defp filter_unseen_events(events, seen_ids) do
    # Filter to events we haven't seen yet
    # Note: events have atom keys from Memgraph module (%{id: ..., ...})
    new_events = Enum.filter(events, fn event ->
      event_id = Map.get(event, :id) || Map.get(event, "id")
      event_id && !MapSet.member?(seen_ids, event_id)
    end)

    # Add new event IDs to seen set
    new_ids = new_events
      |> Enum.map(fn e -> Map.get(e, :id) || Map.get(e, "id") end)
      |> Enum.filter(& &1)
      |> MapSet.new()

    updated_seen = MapSet.union(seen_ids, new_ids)

    # Trim set if it gets too large (keep most recent by converting to list and back)
    updated_seen = if MapSet.size(updated_seen) > @max_seen_ids do
      updated_seen
      |> MapSet.to_list()
      |> Enum.take(-@max_seen_ids)
      |> MapSet.new()
    else
      updated_seen
    end

    {new_events, updated_seen}
  end
end
