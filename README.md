should_schedule will run the specified job after a certain numbers of hours from the last job end. 
At the moment this task will drift because second run wait will start after first run completes.

Need to add better database error handling.

We currently reattempt to connect on every database error. Needs to be more granular so that only connection errors will trigger a retry 