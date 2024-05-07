use std::{collections::HashMap, env, fs, time::Duration};

use chrono::{DateTime, Days, NaiveTime};
use dotenvy::dotenv;
use reqwest::Client;
use serde_json::Value;
use sqlx::{Connection, SqliteConnection};

#[allow(dead_code)]
struct UserLog {
    id: i64,
    user_id: String,
    channel_id: String,
    join_time: Option<i64>,
    leave_time: Option<i64>,
    duration: Option<i64>
}

struct TrackedUser {
    name: String,
    image: String,
}

#[tokio::main]
async fn main() {
    println!("brrr");

    dotenv().expect(".env file is missing");
    let varibles: HashMap<String, String> = env::vars().collect();

    let discord_token = varibles.get("DISCORD_TOKEN").expect("there is no DISCORD_TOKEN in env");

    let mut conn: SqliteConnection = SqliteConnection::connect("sqlite://metric.db").await.unwrap();

    let response = sqlx::query_as!(UserLog, "SELECT * FROM users")
        .fetch_all(&mut conn)
        .await.unwrap();

    let first = response.first().unwrap();
    let last = response.last().unwrap();

    let start_date = DateTime::from_timestamp_millis(first.join_time.unwrap()).unwrap().with_time(NaiveTime::MIN).unwrap();
    let end_date = DateTime::from_timestamp_millis(last.leave_time.unwrap()).unwrap();
    
    let web_client = Client::new();

    let mut all_users = HashMap::new();
    for user in &response {
        let user_id = user.user_id.clone();
        if all_users.contains_key(&user_id) {
            continue;
        }

        let json: Value = web_client.get(format!("https://canary.discord.com/api/v10/users/{}", user_id))
            .header("Authorization", format!("Bot {}", discord_token))
            .send().await.unwrap()
            .json().await.unwrap();

        let user_obj = json.as_object().unwrap();

        //println!("{:?}", user_obj);
        
        let name = user_obj.get("global_name").unwrap().as_str().unwrap_or(user_obj.get("username").unwrap().as_str().unwrap()).to_string();        
        let image = if user_obj.get("avatar").unwrap().as_str().is_some() {
            let avatar_id = user_obj.get("avatar").unwrap().as_str().unwrap().to_string();
            format!("https://cdn.discordapp.com/avatars/{}/{}.webp", user_id, avatar_id)
        } else {
            "https://cdn.discordapp.com/embed/avatars/0.png".to_string()
        };

        println!("hello {}", name);
        
        all_users.insert(user_id, TrackedUser { name, image });

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let mut csv_writer = csv::WriterBuilder::new().from_writer(vec![]);
    csv_writer.write_field("Name").unwrap();
    csv_writer.write_field("Image").unwrap();
    
    let mut local_date = start_date.clone();
    loop {
        csv_writer.write_field(local_date.date_naive().to_string()).unwrap();

        if local_date > end_date {
            csv_writer.write_record(None::<&[u8]>).unwrap();
            break;
        }
        local_date = local_date.checked_add_days(Days::new(1)).unwrap();
    }

    for (user_id, tracked_user) in all_users {
        let mut local_date = start_date.clone();

        csv_writer.write_field(tracked_user.name).unwrap();
        csv_writer.write_field(tracked_user.image).unwrap();

        loop {
            let mut counter: i64 = 0;
            for user in &response {
                if user.user_id == user_id {
                    if DateTime::from_timestamp_millis(user.leave_time.unwrap()).unwrap() > local_date {
                        break;
                    }

                    counter += user.duration.unwrap();
                }
            }

            csv_writer.write_field(counter.to_string()).unwrap();

            if local_date > end_date {
                csv_writer.write_record(None::<&[u8]>).unwrap();
                break;
            }
            local_date = local_date.checked_add_days(Days::new(1)).unwrap();
        }
    }

    csv_writer.flush().unwrap();
    fs::write("temp.csv", csv_writer.get_ref()).unwrap();
}
