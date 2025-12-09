#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = ["neo4j>=5.0"]
# ///
"""Test script to query Memgraph for Session Work feature and events."""

from neo4j import GraphDatabase

def main():
    driver = GraphDatabase.driver('bolt://localhost:7687', auth=('', ''))

    with driver.session(database='memgraph') as session:
        # Check for Session Work feature
        result = session.run('''
            MATCH (f:Feature)
            WHERE f.is_session_work = true
            RETURN f.description as description, f.work_count as work_count
        ''')
        records = list(result)
        print(f'Session Work features: {len(records)}')
        for r in records:
            print(f'  - {r["description"]}: work_count={r["work_count"]}')

        # Check for events
        result = session.run('''
            MATCH (e:Event)
            RETURN e.event_type as event_type, e.tool_name as tool_name
            LIMIT 10
        ''')
        records = list(result)
        print(f'Events: {len(records)}')
        for r in records:
            print(f'  - {r["event_type"]}: {r["tool_name"]}')

        # Count all features
        result = session.run('MATCH (f:Feature) RETURN count(f) as count')
        count = result.single()["count"]
        print(f'\nTotal features: {count}')

    driver.close()

if __name__ == "__main__":
    main()
