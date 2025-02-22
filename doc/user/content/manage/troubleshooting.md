---
title: "Troubleshooting"
description: "Troubleshoot issues."
menu:
  main:
    parent: manage
    weight: 5
aliases:
  - /ops/diagnosing-using-sql/
  - /ops/troubleshooting/
---

This page describes several SQL queries you can run to diagnose performance issues with Materialize.


## Mental model and basic terminology

When you create a materialized view, an index, or issue an ad-hoc query, Materialize creates a so-called dataflow. A dataflow consists of instructions on how to respond to data input and to changes to that data. Once executed, the dataflow computes the result of the SQL query, waits for updates from the sources, and then incrementally updates the query results when new data arrives.

Materialize dataflows act on collections of data. To provide fast access to the changes to individual records, the records can be stored in an indexed representation called [arrangements](https://materialize.com/docs/get-started/arrangements/#arrangements). Arrangements can be manually created by users on views by creating an index on the view. But they are also used internally in dataflows, for instance, when joining relations.


### Translating SQL to dataflows

To make these concepts a bit more tangible, let's look at the example from the [getting started guide](https://materialize.com/docs/get-started/quickstart/).

```sql
CREATE SOURCE auction_house
  FROM LOAD GENERATOR AUCTION
  (TICK INTERVAL '100ms')
  FOR ALL TABLES;

CREATE MATERIALIZED VIEW num_bids AS
  SELECT auctions.item, count(bids.id) AS number_of_bids
  FROM bids
  JOIN auctions ON bids.auction_id = auctions.id
  WHERE bids.bid_time < auctions.end_time
  GROUP BY auctions.item;

CREATE INDEX num_bids_idx ON num_bids (item);
```

The query of the materialized view joins the relations `bids` and `auctions`, groups by `auctions.item` and determines the number of bids per auction. To understand how this SQL query is translated to a dataflow, we can use [`EXPLAIN PLAN`](https://materialize.com/docs/sql/explain-plan/) to display the plan used to evaluate the join.

```sql
EXPLAIN MATERIALIZED VIEW num_bids;
```
```
                    Optimized Plan
-------------------------------------------------------
 materialize.public.num_bids:                         +
   Reduce group_by=[#0] aggregates=[count(*)]         +
     Project (#3)                                     +
       Filter (#1 < #4)                               +
         Join on=(#0 = #2) type=differential          +
           ArrangeBy keys=[[#0]]                      +
             Project (#2, #4)                         +
               ReadStorage materialize.public.bids    +
           ArrangeBy keys=[[#0]]                      +
             Project (#0, #2, #3)                     +
               ReadStorage materialize.public.auctions+

(1 row)
```

The plan describes the specific operators that are used to evaluate the query.
Some of these operators resemble relational algebra or map reduce style operators (`Filter`, `Join`, `Project`).
Others are specific to Materialize (`ArrangeBy`, `ReadStorage`).

In general, a high level understanding of what these operators do is sufficient for effective debugging:
`Filter` filters records, `Join` joins records from two or more inputs, `Map` applies a function to transform records, etc.
You can find more details on these operators in the [`EXPLAIN PLAN` documentation](https://materialize.com/docs/sql/explain-plan/#operators-in-decorrelated-and-optimized-plans).
But it's not important to have a deep understanding of all these operators for effective debugging.


Behind the scenes, the operator graph is turned into a dataflow.
The dataflow is organized in a hierarchical structure that contains operators and _regions_, which define a hierarchy on the operators.
In our example, the dataflow contains an `InputRegion`, a `BuildRegion`, and a region for the sink.

![Regions and operator visualization](/images/regions-and-operators-hierarchy.png)

Again, it's not too important for our purposes to understand what these regions do and how they are used to structure the operator graph.
For our purposes it's just important to know than that they define a hierarchy on the operators.


## The system catalog and introspection relations

Materialize collects a lot of useful information about the dataflows and operators in the system catalog in [introspection relations](/sql/system-catalog/mz_internal/#replica-introspection-relations).
The introspection relations are useful to troubleshoot and understand what is happening under the hood when Materialize is not behaving as expected.
However, it is important to understand that most of the statistics we need for troubleshooting purposes are specific to the cluster that is running the queries we want to debug.

{{< warning >}}
Indexes and dataflows are local to a cluster, so their introspection information will vary across clusters depending on the active cluster and replica.
As a consequence, you should expect the results of the queries below to vary depending on the values set for the `cluster` and `cluster_replica` [session variables](/sql/set/#session-variables).
{{< /warning >}}

<!--
[//]: # "TODO(joacoc) We should share ways for the user to diagnose and troubleshoot if and how fast a source is consuming."
``` -->

## Where is Materialize spending compute time?

Materialize spends time in various dataflow operators.
Dataflows are created to maintain materialized views or indexes.
In addition, temporary dataflow will be created for ad-hoc queries that don't just make a lookup to an existing index.

If Materialize is taking more time to update
results than you expect, you can identify which operators
take the largest total amount of time.

### Identifying expensive dataflows

To understand which dataflow is taking the most time we can query the `mz_scheduling_elapsed` relation.
The `elapsed_time` metric shows the absolute time the dataflows was busy since the system started and the dataflow was created.

```sql
-- Extract raw elapsed time information for dataflows
SELECT
    mdo.id,
    mdo.name,
    mse.elapsed_ns / 1000 * '1 MICROSECONDS'::interval AS elapsed_time
FROM mz_internal.mz_scheduling_elapsed AS mse,
    mz_internal.mz_dataflow_operators AS mdo,
    mz_internal.mz_dataflow_addresses AS mda
WHERE mse.id = mdo.id AND mdo.id = mda.id AND list_length(address) = 1
ORDER BY elapsed_ns DESC
```
```
 id  |                  name                  |  elapsed_time
-----+----------------------------------------+-----------------
 354 | Dataflow: materialize.qck.num_bids     | 02:05:25.756836
 578 | Dataflow: materialize.qck.num_bids_idx | 00:15:04.838741
 (2 rows)
```

These results show that Materialize spends the most time keeping the materialized view `num_bids` up to date,
followed by the work on the index `avg_bids_idx`.

### Identifying expensive operators in a dataflow

The previous query is a good starting point to get an overview of the work happening in the cluster because it only returns dataflows.

The `mz_scheduling_elapsed` relation also contains details for regions and operators.
Removing the condition `list_length(address) = 1` will include the regions and operators in the result.
But be aware that every row shows aggregated times for all the elements it contains.
The `elapsed_time` reported for the dataflows above also includes the elapsed time for all the regions and operators they contain.

But because the parent-child relationship is not always obvious,
the results containing a mixture of dataflows, regions, and operators can be a bit hard to interpret.
The following query therefore only returns operators from the  `mz_scheduling_elapsed` relation.
You can further drill down by adding a filter condition that matches the name of a specific dataflow.

```sql
SELECT
    mdod.id,
    mdod.name,
    mdod.dataflow_name,
    mse.elapsed_ns / 1000 * '1 MICROSECONDS'::interval AS elapsed_time
FROM mz_internal.mz_scheduling_elapsed AS mse,
    mz_internal.mz_dataflow_addresses AS mda,
    mz_internal.mz_dataflow_operator_dataflows AS mdod
WHERE
    mse.id = mdod.id AND mdod.id = mda.id
    -- exclude regions and just return operators
    AND mda.address NOT IN (
        SELECT DISTINCT address[:list_length(address) - 1]
        FROM mz_internal.mz_dataflow_addresses
    )
ORDER BY elapsed_ns DESC
```
```
 id  |                      name                       |             dataflow_name              |  elapsed_time
-----+-------------------------------------------------+----------------------------------------+-----------------
 431 | ArrangeBy[[Column(0)]]                          | Dataflow: materialize.qck.num_bids     | 01:12:58.964875
 442 | ArrangeBy[[Column(0)]]                          | Dataflow: materialize.qck.num_bids     | 00:06:06.080178
 528 | shard_source_fetch(u517)                        | Dataflow: materialize.qck.num_bids     | 00:04:04.076344
 594 | shard_source_fetch(u517)                        | Dataflow: materialize.qck.num_bids_idx | 00:03:34.803234
 590 | persist_source_backpressure(backpressure(u517)) | Dataflow: materialize.qck.num_bids_idx | 00:03:33.626036
 400 | persist_source_backpressure(backpressure(u510)) | Dataflow: materialize.qck.num_bids     | 00:03:03.575832
...
```

From the results of this query we can see that most of the elapsed time of the dataflow is caused by the time it takes to maintain the updates in one of the arrangements of the dataflow of the materialized view.

## Why is Materialize unresponsive or slow?
Slow responses can be the result of a number of factors, which we can help you understand and address.

If you haven't already, you should consider an [index](https://materialize.com/docs/sql/create-index/) for your query, and check out the [optimizations](https://materialize.com/docs/transform-data/optimization/) on how to optimize query performance in Materialize.

You can also consider changing your isolation level, depending on the [consistency guarantees](https://materialize.com/docs/get-started/isolation-level) that you need.

### Transactions

Transactions are a database concept for bundling multiple query steps into a single, all-or-nothing operation. You can read more about them in the [transactions](https://materialize.com/docs/sql/begin) section of our docs.

In Materialize, `BEGIN` starts a transaction block. All statements in a transaction block will be executed in a single transaction until an explicit `COMMIT` or `ROLLBACK` is given. All statements in that transaction happen at the same timestamp, and that timestamp must be valid for all objects the transaction may access.

What this means for latency: Materialize may delay queries against "slow" tables, materialized views, and indexes until they catch up to faster ones in the same schema. We recommend you avoid using transactions in contexts where you require low latency responses and are not certain that all objects in a schema will be equally current.

What you can do:

- Avoid using transactions where you don’t need them. For example, if you’re only executing single statements at a time.
- Double check whether your SQL library or ORM is wrapping all queries in transactions on your behalf, and disable that setting, only using transactions explicitly when you want them.

### Client-side latency

To minimize the roundtrip latency associated with making requests from your client to Materialize, make your requests as physically close to your Materialize region as possible. For example, if you use the AWS `us-east-1` region for Materialize, your client server would ideally also be running in AWS `us-east-1`.

### Debugging expensive dataflows and operators

A large class of problems can be identified by using [`elapsed_time`](#identifying-expensive-operators-in-a-dataflow) to estimate the most expensive dataflows and operators.
However, `elapsed_time` contains all work since the operator or dataflow was first created.
Sometimes, a lot of work happens initially when the operator is created, but later on it takes only little continuous effort.
If you want to see what operator is taking the most time **right now**,
the `elapsed_time` metric is not enough.

The relation `mz_compute_operator_durations_histogram` also tracks the time operators are busy,
but instead of aggregating `elapsed_time` since an operator got created,
it tracks how long each operator was scheduled at a time in a histogram.
This information can show you two things: operators that block progress for others and operators that are currently doing work.

If there is a very expensive operator that blocks progress for all other operators,
it will become visible in the histogram.
The offending operator will be scheduled in much longer intervals compared to other operators, which reflects in the histogram as larger time buckets.

```sql
-- Extract raw scheduling histogram information for operators
WITH histograms AS (
    SELECT
        mdod.id,
        mdod.name,
        mdod.dataflow_name,
        mcodh.count,
        mcodh.duration_ns / 1000 * '1 MICROSECONDS'::interval AS duration
    FROM mz_internal.mz_compute_operator_durations_histogram AS mcodh,
        mz_internal.mz_dataflow_addresses AS mda,
        mz_internal.mz_dataflow_operator_dataflows AS mdod
    WHERE
        mcodh.id = mdod.id
        AND mdod.id = mda.id
        -- exclude regions and just return operators
        AND mda.address NOT IN (
            SELECT DISTINCT address[:list_length(address) - 1]
            FROM mz_internal.mz_dataflow_addresses
        )

)

SELECT *
FROM histograms
WHERE duration > '100 millisecond'::interval
ORDER BY duration DESC
```
```
 id  |                 name                 |             dataflow_name              | count |    duration
-----+--------------------------------------+----------------------------------------+-------+-----------------
 408 | persist_source::decode_and_mfp(u510) | Dataflow: materialize.qck.num_bids     |     1 | 00:00:01.073741
 408 | persist_source::decode_and_mfp(u510) | Dataflow: materialize.qck.num_bids     |     1 | 00:00:00.53687
 374 | persist_source::decode_and_mfp(u509) | Dataflow: materialize.qck.num_bids     |     1 | 00:00:00.268435
 408 | persist_source::decode_and_mfp(u510) | Dataflow: materialize.qck.num_bids     |     2 | 00:00:00.268435
 429 | FormArrangementKey                   | Dataflow: materialize.qck.num_bids     |     2 | 00:00:00.134217
 (5 rows)
```

Note that this relation contains a lot of information.
The query therefore filters all durations below `100 millisecond`.
In this example, the result of the query shows that the longest an operator got scheduled is just over one second.
So it's unlikely that Materialize is unresponsive because of a single operator blocking progress for others.

The reported duration is still reporting aggregated values since the operator has been created.
To get a feeling for which operators are currently doing work,
you can subscribe to the changes of the relation.

```sql
-- Observe changes to the raw scheduling histogram information
COPY(SUBSCRIBE(
    WITH histograms AS (
        SELECT
            mdod.id,
            mdod.name,
            mdod.dataflow_name,
            mcodh.count,
            mcodh.duration_ns / 1000 * '1 MICROSECONDS'::interval AS duration
        FROM mz_internal.mz_compute_operator_durations_histogram AS mcodh,
            mz_internal.mz_dataflow_addresses AS mda,
            mz_internal.mz_dataflow_operator_dataflows AS mdod
        WHERE
            mcodh.id = mdod.id
            AND mdod.id = mda.id
            -- exclude regions and just return operators
            AND mda.address NOT IN (
                SELECT DISTINCT address[:list_length(address) - 1]
                FROM mz_internal.mz_dataflow_addresses
            )

    )

    SELECT *
    FROM histograms
    WHERE duration > '100 millisecond'::interval
) WITH (SNAPSHOT = false, PROGRESS)) TO STDOUT
```
```
1691667343000	t	\N	\N	\N	\N	\N	\N
1691667344000	t	\N	\N	\N	\N	\N	\N
1691667344000	f	-1	431	ArrangeBy[[Column(0)]]	Dataflow: materialize.qck.num_bids	7673	00:00:00.104800
1691667344000	f	1	431	ArrangeBy[[Column(0)]]	Dataflow: materialize.qck.num_bids	7674	00:00:00.104800
1691667345000	t	\N	\N	\N	\N	\N	\N
...
```

In this way you can see that currently the only operator that is doing more than 100 milliseconds worth of work
is the `ArrangeBy` operator from the materialized view `num_bids`.


## Why is Materialize using so much memory?

[Arrangements](/overview/arrangements)
take up most of Materialize's memory use. Arrangements maintain indexes for data
as it changes. These queries extract the numbers of records and
the size of the arrangements. The reported records may
exceed the number of logical records; the report reflects the uncompacted
state.

```sql
-- Extract dataflow records and sizes
SELECT
    id,
    name,
    records,
    round(size / 1024 / 1024, 2) AS size_mb
FROM mz_internal.mz_dataflow_arrangement_sizes
ORDER BY size DESC
```
```
  id   |                  name                  | records | size_mb
-------+----------------------------------------+---------+---------
 19157 | Dataflow: materialize.qck.num_bids     | 1612747 |  113.82
 19158 | Dataflow: materialize.qck.num_bids_idx |      13 |       0
```

If you need to drill down into individual operators, you can query `mz_arrangement_sizes` instead.

```sql
-- Extract operator records and sizes
SELECT
    mdod.id,
    mdod.name,
    mdod.dataflow_name,
    mas.records,
    round(size / 1024 / 1024, 2) AS size_mb
FROM mz_internal.mz_arrangement_sizes AS mas,
    mz_internal.mz_dataflow_operator_dataflows AS mdod
WHERE mas.operator_id = mdod.id
ORDER BY mas.records DESC
```
```
   id    |             name              |             dataflow_name              | records | size_mb
---------+-------------------------------+----------------------------------------+---------+---------
 2722012 | ArrangeBy[[Column(0)]]        | Dataflow: materialize.qck.num_bids     | 1612747 |  113.82
 2722027 | ArrangeBy[[Column(0)]]        | Dataflow: materialize.qck.num_bids     |  292662 |   20.65
 2722216 | ArrangeBy[[Column(0)]]        | Dataflow: materialize.qck.num_bids_idx |      17 |       0
 2722077 | ReduceAccumulable             | Dataflow: materialize.qck.num_bids     |       5 |       0
 2722073 | ArrangeAccumulable            | Dataflow: materialize.qck.num_bids     |       5 |       0
 2722081 | AccumulableErrorCheck         | Dataflow: materialize.qck.num_bids     |       0 |       0
 2722225 | ArrangeBy[[Column(0)]]-errors | Dataflow: materialize.qck.num_bids_idx |       0 |       0
(7 rows)
```

We've also bundled an interactive, web-based memory usage visualization tool to
aid in debugging. The memory visualization tool shows all user-created arrangements,
grouped by dataflow. The amount of memory used
by Materialize should correlate with the number of arrangement records that are
displayed by either the visual interface or the SQL queries.

You can access the memory usage visualization for your Materialize region at
`https://<region host>/memory`.

## Is work distributed equally across workers?

Work is distributed across workers by the hash of their keys. Thus, work can
become skewed if situations arise where Materialize needs to use arrangements
with very few or no keys. Example situations include:

* Views that maintain order by/limit/offset
* Cross joins
* Joins where the join columns have very few unique values

Additionally, the operators that implement data sources may demonstrate skew, as
they (currently) have a granularity determined by the source itself. For
example, Kafka topic ingestion work can become skewed if most of the data is in
only one out of multiple partitions.

```sql
-- Get operators where one worker has spent more than 2 times the average
-- amount of time spent. The number 2 can be changed according to the threshold
-- for the amount of skew deemed problematic.
SELECT
    mse.id,
    dod.name,
    mse.worker_id,
    elapsed_ns,
    avg_ns,
    elapsed_ns/avg_ns AS ratio
FROM
    mz_internal.mz_scheduling_elapsed_per_worker mse,
    (
        SELECT
            id,
            avg(elapsed_ns) AS avg_ns
        FROM
            mz_internal.mz_scheduling_elapsed_per_worker
        GROUP BY
            id
    ) aebi,
    mz_internal.mz_dataflow_operator_dataflows dod
WHERE
    mse.id = aebi.id AND
    mse.elapsed_ns > 2 * aebi.avg_ns AND
    mse.id = dod.id
ORDER BY ratio DESC;
```

## I found a problematic operator. Where did it come from?

Look up the operator in `mz_dataflow_addresses`. If an operator has
value `x` at position `n`, then it is part of the `x` subregion of the region
defined by positions `0..n-1`. The example SQL query and result below shows an
operator whose `id` is 515 that belongs to "subregion 5 of region 1 of dataflow
21".
```sql
SELECT * FROM mz_internal.mz_dataflow_addresses WHERE id=515;
```
```
 id  | worker_id | address
-----+-----------+----------
 515 |      0    | {21,1,5}
```

Usually, it is only important to know the name of the dataflow a problematic
operator comes from. Once the name is known, the dataflow can be correlated to
an index or materialized view in Materialize.

Each dataflow has an operator representing the entire dataflow. The address of
said operator has only a single entry. For the example operator 515 above, you
can find the name of the dataflow if you can find the name of the operator whose
address is just "dataflow 21."

```sql
-- get id and name of the operator representing the entirety of the dataflow
-- that a problematic operator comes from
SELECT
    mdo.id AS id,
    mdo.name AS name
FROM
    mz_internal.mz_dataflow_addresses mda,
    -- source of operator names
    mz_internal.mz_dataflow_operators mdo,
    -- view containing operators representing entire dataflows
    (SELECT
      mda.id AS dataflow_operator,
      mda.address[1] AS dataflow_address
    FROM
      mz_internal.mz_dataflow_addresses mda
    WHERE
      list_length(mda.address) = 1) dataflows
WHERE
    mda.id = 515
    AND mda.address[1] = dataflows.dataflow_address
    AND mdo.id = dataflows.dataflow_operator;
```

## Why isn't my source ingesting data?

First, look for errors in [`mz_source_statuses`](/sql/system-catalog/mz_internal/#mz_source_statuses):

```sql
SELECT * FROM mz_internal.mz_source_statuses
WHERE name = <SOURCE_NAME>;
```

If your source reports a status of `stalled` or `failed`, you likely have a
configuration issue. The returned `error` field will provide details.

If your source reports a status of `starting` for more than a few minutes,
[contact support](/support).

If your source reports a status of `running`, but you are not receiving data
when you query the source, the source may still be ingesting its initial
snapshot. See the next section.

## Has my source ingested its initial snapshot?

Query the `snapshot_committed` field of the
[`mz_source_statistics`](/sql/system-catalog/mz_internal/#mz_source_statistics) table:

```sql
SELECT snapshot_committed
FROM mz_internal.mz_source_statistics
WHERE id = <SOURCE ID>;
```

You generally want to aggregate the `snapshot_committed` field across all worker
threads, as done in the above query. The snapshot is only considered committed
for the source as a whole once all worker threads have committed their
components of the snapshot.

Even if your source has not yet committed its initial snapshot, you can still
monitor its progress. See the next section.

## How do I monitor source ingestion progress?

Repeatedly query the
[`mz_source_statistics`](/sql/system-catalog/mz_internal/#mz_source_statistics)
table and look for ingestion statistics that advance over time:

```sql
SELECT
    bytes_received,
    messages_received,
    updates_staged,
    updates_committed
FROM mz_internal.mz_source_statistics
WHERE id = <SOURCE ID>;
```

(You can also look at statistics for individual worker threads to evaluate
whether ingestion progress is skewed, but it's generally simplest to start
by looking at the aggregate statistics for the whole source.)

The `bytes_received` and `messages_received` statistics should roughly
correspond with the external system's measure of progress. For example, the
`bytes_received` and `messages_received` fields for a Kafka source should
roughly correspond to what the upstream Kafka broker reports as the number of
bytes (including the key) and number of messages transmitted, respectively.

During the initial snapshot, `updates_committed` will remain at zero until all
messages in the snapshot have been staged. Only then will `updates_committed`
advance. This is expected, and not a cause for concern.

After the initial snapshot, there should be relatively little skew between
`updates_staged` and `updates_committed`. A large gap is usually an indication
that the source has fallen behind, and that you likely need to scale up your
source.

`messages_received` does not necessarily correspond with `updates_staged`
and `updates_commmited`. For example, an `UPSERT` envelope can have _more_
updates than messages, because messages can cause both deletions and insertions
(i.e. when they update a value for a key), which are both counted in the
`updates_*` statistics. There can also be _fewer_ updates than messages, as
many messages for a single key can be consolidated if they occur within a (small)
internally configured window. That said, `messages_received` making
steady progress, while `updates_staged`/`updates_committed` doesn't is also
evidence that a source has fallen behind, and may need to be scaled up.

Beware that these statistics periodically reset to zero, as internal components
of the system restart. This is expected behavior. As a result, you should
restrict your attention to how these statistics evolve over time, and not their
absolute values at any moment in time.

## Why isn't my sink exporting data?
First, look for errors in [`mz_sink_statuses`](/sql/system-catalog/mz_internal/#mz_sink_statuses):

```sql
SELECT * FROM mz_internal.mz_sink_statuses
WHERE name = <SINK_NAME>;
```

If your sink reports a status of `stalled` or `failed`, you likely have a
configuration issue. The returned `error` field will provide details.

If your sink reports a status of `starting` for more than a few minutes,
[contact support](/support).

## How do I monitor sink ingestion progress?

Repeatedly query the
[`mz_sink_statistics`](/sql/system-catalog/mz_internal/#mz_sink_statistics)
table and look for ingestion statistics that advance over time:

```sql
SELECT
    messages_staged,
    messages_committed,
    bytes_staged,
    bytes_committed
FROM mz_internal.mz_sink_statistics
WHERE id = <SINK ID>;
```

(You can also look at statistics for individual worker threads to evaluate
whether ingestion progress is skewed, but it's generally simplest to start
by looking at the aggregate statistics for the whole source.)

The `messages_staged` and `bytes_staged` statistics should roughly correspond
with what materialize has written (but not necessarily committed) to the
external service. For example, the `bytes_staged` and `messages_staged` fields
for a Kafka sink should roughly correspond with how many messages materialize
has written to the Kafka topic, and how big they are (including the key), but
the Kafka transaction for those messages might not have been committed yet.

`messages_committed` and `bytes_committed` correspond to the number of messages
committed to the external service. These numbers can be _smaller_ than the
`*_staged` statistics, because Materialize might fail to write transactions and
retry them.

If any of these statistics are not making progress, your sink might be stalled
or need to be scaled up.

If the `*_staged` statistics are making progress, but the `*_committed` ones
are not, there may be a configuration issues with the external service that is
preventing Materialize from committing transactions. Check the `reason`
column in `mz_sink_statuses`, which can provide more information.
