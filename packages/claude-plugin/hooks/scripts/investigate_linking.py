#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = ["neo4j>=5.0"]
# ///
"""Investigate event-to-feature linking issues."""

from neo4j import GraphDatabase

def main():
    driver = GraphDatabase.driver('bolt://localhost:7687', auth=('', ''))
    project_path = '/Users/shakes/DevProjects/ijoka'

    with driver.session(database='memgraph') as session:
        # 1. Find all in_progress features
        print("=" * 60)
        print("FEATURES WITH status='in_progress'")
        print("=" * 60)
        result = session.run("""
            MATCH (f:Feature {status: 'in_progress'})-[:BELONGS_TO]->(p:Project {path: $projectPath})
            RETURN f.id as id, f.description as description, f.is_session_work as is_session_work
            ORDER BY f.created_at
        """, {"projectPath": project_path})

        for r in result:
            session_work = "✓ Session Work" if r['is_session_work'] else ""
            print(f"  [{r['id'][:8]}...] {r['description'][:50]}... {session_work}")

        # 2. Find the "Claude plugin installs" feature
        print("\n" + "=" * 60)
        print("LOOKING FOR 'Claude plugin' FEATURES")
        print("=" * 60)
        result = session.run("""
            MATCH (f:Feature)-[:BELONGS_TO]->(p:Project {path: $projectPath})
            WHERE f.description CONTAINS 'Claude plugin' OR f.description CONTAINS 'plugin installs'
            RETURN f.id as id, f.description as description, f.status as status
        """, {"projectPath": project_path})

        records = list(result)
        if records:
            for r in records:
                print(f"  [{r['status']}] {r['description'][:60]}...")
        else:
            print("  (not found in graph - might be in SQLite)")

        # 3. Check all events and their linked features
        print("\n" + "=" * 60)
        print("ALL EVENTS AND THEIR LINKED FEATURES")
        print("=" * 60)
        result = session.run("""
            MATCH (e:Event)
            OPTIONAL MATCH (e)-[:LINKED_TO]->(f:Feature)
            OPTIONAL MATCH (e)-[:BELONGS_TO]->(f2:Feature)
            RETURN e.id as event_id, e.event_type as event_type, e.tool_name as tool_name,
                   f.description as linked_feature, f2.description as belongs_to_feature
            ORDER BY e.timestamp DESC
            LIMIT 20
        """)

        for r in result:
            linked = r['linked_feature'] or r['belongs_to_feature'] or "(no feature)"
            print(f"  {r['event_type']}:{r['tool_name']}")
            print(f"     → {linked[:50]}...")

        # 4. Check the Session Work feature specifically
        print("\n" + "=" * 60)
        print("SESSION WORK FEATURE DETAILS")
        print("=" * 60)
        result = session.run("""
            MATCH (f:Feature {is_session_work: true})-[:BELONGS_TO]->(p:Project {path: $projectPath})
            OPTIONAL MATCH (e:Event)-[:BELONGS_TO]->(f)
            WITH f, count(e) as event_count
            RETURN f.id as id, f.description as description, f.status as status, event_count
        """, {"projectPath": project_path})

        records = list(result)
        if records:
            for r in records:
                print(f"  ID: {r['id']}")
                print(f"  Description: {r['description']}")
                print(f"  Status: {r['status']}")
                print(f"  Events linked: {r['event_count']}")
        else:
            print("  (Session Work feature not found)")

    driver.close()

if __name__ == "__main__":
    main()
