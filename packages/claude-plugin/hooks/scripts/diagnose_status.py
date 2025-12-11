#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = ["neo4j>=5.0"]
# ///
"""Diagnose feature status vs activity relationships."""

from neo4j import GraphDatabase

def main():
    driver = GraphDatabase.driver('bolt://localhost:7687', auth=('', ''))

    with driver.session(database='memgraph') as session:
        # 1. Feature status distribution
        print("=" * 60)
        print("FEATURE STATUS DISTRIBUTION")
        print("=" * 60)
        result = session.run('''
            MATCH (f:Feature)-[:BELONGS_TO]->(p:Project)
            RETURN f.status as status, count(f) as count
            ORDER BY count DESC
        ''')
        for r in result:
            print(f"  {r['status']}: {r['count']}")

        # 2. Features with events (activity) but still pending
        print("\n" + "=" * 60)
        print("FEATURES WITH ACTIVITIES BUT STILL PENDING")
        print("=" * 60)
        result = session.run('''
            MATCH (f:Feature {status: 'pending'})-[:BELONGS_TO]->(p:Project)
            OPTIONAL MATCH (e:Event)-[:BELONGS_TO]->(f)
            WITH f, count(e) as event_count
            WHERE event_count > 0
            RETURN f.description as description, f.status as status,
                   f.work_count as work_count, event_count
            ORDER BY event_count DESC
        ''')
        records = list(result)
        if records:
            for r in records:
                print(f"  [{r['status']}] {r['description'][:50]}...")
                print(f"           events: {r['event_count']}, work_count: {r['work_count']}")
        else:
            print("  (none found)")

        # 3. All features with their activity counts
        print("\n" + "=" * 60)
        print("ALL FEATURES WITH ACTIVITY COUNTS")
        print("=" * 60)
        result = session.run('''
            MATCH (f:Feature)-[:BELONGS_TO]->(p:Project)
            WHERE f.is_session_work IS NULL OR f.is_session_work = false
            OPTIONAL MATCH (e:Event)-[:BELONGS_TO]->(f)
            WITH f, count(e) as event_count
            RETURN f.description as description, f.status as status,
                   f.work_count as work_count, event_count, f.category as category
            ORDER BY event_count DESC, f.priority DESC
            LIMIT 25
        ''')
        for r in result:
            status_icon = {"pending": "⬜", "in_progress": "🔵", "complete": "✅", "blocked": "🔴"}.get(r['status'], "?")
            print(f"  {status_icon} [{r['status']:12}] {r['description'][:45]}...")
            print(f"                    events: {r['event_count']}, work: {r['work_count']}, cat: {r['category']}")

        # 4. Check for StatusEvent nodes (temporal pattern)
        print("\n" + "=" * 60)
        print("STATUS EVENTS (Temporal Pattern)")
        print("=" * 60)
        result = session.run('''
            MATCH (se:StatusEvent)
            RETURN se.from_status as from_status, se.to_status as to_status,
                   se.at as at, se.by as by
            LIMIT 10
        ''')
        records = list(result)
        if records:
            for r in records:
                print(f"  {r['from_status']} → {r['to_status']} at {r['at']} by {r['by']}")
        else:
            print("  (no StatusEvent nodes found - temporal pattern not implemented)")

        # 5. Event-to-Feature links
        print("\n" + "=" * 60)
        print("EVENT-TO-FEATURE LINKS")
        print("=" * 60)
        result = session.run('''
            MATCH (e:Event)-[r:BELONGS_TO]->(f:Feature)
            RETURN e.event_type as event_type, e.tool_name as tool_name,
                   f.description as feature, type(r) as rel_type
            LIMIT 15
        ''')
        records = list(result)
        if records:
            for r in records:
                print(f"  {r['event_type']}:{r['tool_name']} → {r['feature'][:40]}...")
        else:
            print("  (no event-to-feature links found)")

    driver.close()

if __name__ == "__main__":
    main()
