import { Module } from '@nestjs/common';
import { AppConfigModule } from './config/app-config.module';
import { AppController } from './app.controller';
import { AppService } from './app.service';
import { MarketsModule } from './markets/markets.module';
import { PrismaModule } from './prisma/prisma.module';
import { RelayerModule } from './relayer/relayer.module';
import { VaultsModule } from './vaults/vaults.module';
import { VotesModule } from './votes/votes.module';
import { WebhooksModule } from './webhooks/webhooks.module';

@Module({
  imports: [
    AppConfigModule,
    PrismaModule,
    WebhooksModule,
    VaultsModule,
    VotesModule,
    RelayerModule,
    MarketsModule,
  ],
  controllers: [AppController],
  providers: [AppService],
})
export class AppModule {}
