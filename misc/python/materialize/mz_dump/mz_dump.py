# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.


import psycopg2
import sqlparse
from pygments import highlight
from pygments.lexers import SqlLexer
from pygments.formatters import TerminalFormatter

import os
import argparse 

# Parse command-line arguments
parser = argparse.ArgumentParser(
    description='mz_dump: a very minimal pg_dump clone for Materialize',
    formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
parser.add_argument('--user', help='Materialize user', required=True)
parser.add_argument('--host', help='Hostname', required=True)
parser.add_argument('--color', help='Enable colorized SQL output', action='store_true')
parser.add_argument('--password', help='Password (if not in .pgpass file)', required=False, default=None)
parser.add_argument('--port', help='Port', required=False, default=6875)
parser.add_argument('--database', help='Database name', required=False, default='materialize')
parser.add_argument('--sslmode', help='SSL mode (e.g., require, disable)', required=False, default='require')
args = parser.parse_args()

#Materialize connection
conn = psycopg2.connect(
    user=args.user,
    password=args.password,  # This will be None if DB_PASSWORD is not set, relying on .pgpass
    host=args.host,
    port=args.port,
    sslmode=args.sslmode
)

#sql formatter
def highlight_sql(sql):
    formatted_sql = sqlparse.format(sql, reindent=True, keyword_case='upper')
    if args.color: 
        output = highlight(formatted_sql, SqlLexer(), TerminalFormatter())
    else:
        output = formatted_sql
    return output

#query for main loop of table-like objects
def query_createsql_tablelike(table):
    return """
SELECT 
    x.id,
    x.name, 
    x.create_sql, 
    r.name as owner, 
    s.name as schema, 
    d.name as database
FROM 
    materialize.mz_catalog.{} x
JOIN 
    materialize.mz_catalog.mz_roles r on x.owner_id = r.id
JOIN 
    materialize.mz_catalog.mz_schemas s on x.schema_id = s.id
JOIN 
    materialize.mz_catalog.mz_databases d on s.database_id = d.id
WHERE 
    x.owner_id like 'u%'
""".format(table)

#headrer for each object, similar to pg_dump
def header(name, type, database, schema, owner):
    return "--\n-- Name: {}; Type: {};\n-- Database: {}, Schema: {}, Owner: {}\n--\n".format(name,type,database,schema,owner)

#we don't store this sql, so manually recreate per-object ownership statement
def alter_tablelike(type, database, schema, name, owner):
    return "ALTER {} {}.{}.{} OWNER TO {};".format(type, database, schema, name, owner)

#main loop for table-like object
def process_tablelike_object(table, type):
    cur = conn.cursor()
    cur.execute(query_createsql_tablelike(table))
    for result in cur:
        (id, name, sql, owner, schema, database) = result
        print(header(name, type, database, schema, owner))
        print(highlight_sql(sql))
        process_indexes_on_object(id)
        alter = highlight_sql(alter_tablelike(type, database, schema, name, owner))
        print("\n"+alter+"\n")
    cur.close()

#query to find any indexes on the object
def query_indexes_on_object(id):
    return """
SELECT
    create_sql
FROM 
    materialize.mz_catalog.mz_indexes
WHERE 
    on_id = \'{}\'
""".format(id)

#inner loop to process indexes on objects
def process_indexes_on_object(id):
    index_cur = conn.cursor()
    index_cur.execute(query_indexes_on_object(id))
    for result in index_cur:
        index_sql = highlight_sql(result[0])
        print("\n"+index_sql)
    index_cur.close()

#list of table name, object name tuple for each object we will enumerate
tablelike_objects = [
    ('mz_views', 'view'),
    ('mz_tables', 'table'),
    ('mz_sources', 'source'),
    ('mz_sinks', 'sink'),
    ('mz_connections', 'connection'),
    ('mz_materialized_views', 'materialized-view'),
]

#MAIN
for object in tablelike_objects:
    (table, type) = object
    process_tablelike_object(table, type)

##TODO:
# Secrets & Roles

conn.close()